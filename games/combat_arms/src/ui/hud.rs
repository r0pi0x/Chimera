//! Combat Arms' HUD (specs/combat_arms/hud_crosshair.md): the crosshair
//! (shapes, the spread gap, the firing kick, colours, the hit indicator),
//! the damage-taken overlay and direction indicator, the health, armour
//! and stamina block, the weapon and ammo panel, the kill feed, name
//! tags, and the action message (D9: the pickup, plant and defuse
//! prompts; `action`), laid out on the 1024x768 reference canvas
//! stretched to the screen.
//!
//! Behaviour and drawing are apart: `model` is the spec as plain
//! functions; `CaHudStatePlugin` reads the world each frame into `CaHud`
//! (what is shown, where, in which colour, with which text) with no
//! window, so the scenario tests check it headless (tests/ui/hud.rs);
//! `CaHudPlugin` adds it and draws `CaHud` with the shell's UI nodes
//! (with the install's UI data, `CaUi::installed`, the health and weapon
//! panels are the game's HUD layout's, bound by name: `install`; the
//! kill feed, the mode's score panel, the action message and the action
//! gauge too: `bound`, `BoundHud`; the hit overlay, the direction
//! indicator, the gauge sight and the infected's screen, `InfectedTex`
//! while the local player is infected (game_modes.md Corrections
//! (2026-10-30) 3), are the HUD layout attributes' images),
//! telling the shell's fallback HUD (`client::hud::GameDraws`) to leave
//! out its crosshair, health and ammo, hit marker and kill feed, and the
//! shell's minigame callout (`client::callouts`): a playing minigame's
//! callout ("Level up: <gun>", "Level 2/10") is ours, in our notice looks
//! above the middle of the view (under the announcer's kill banner),
//! where the minigame's notices would be
//! (they say the same moments smaller; this HUD leaves them to the
//! panels that list them, CS:S's). Its
//! roots are `client::hud::HudRoot`s and its `GameDraws` in the shell's
//! `HudDraws`, so in an app that switches HUDs it is up only while it is
//! the gameplay's.
//!
//! The kill feed draws another game's gun (a party app's) with that game's
//! icon from `client::weapon_icons::WeaponIcons`, else its display name
//! (`GunNames`); the install's kill icons are registered there for the
//! other games' feeds, its headshot badge as the headshot mark. While the
//! app draws one feed for every HUD (`client::kill_feed::SharedKillFeed`,
//! the party apps) this one stays empty.
//!
//! Inputs: the viewed player (the local one, or whom it watches in first
//! person), its `core::Health`, its armour (`weapon::Armor`: Combat
//! Arms' points, or another game's armour it wears), `Inventory`, `Zoomed`,
//! `Intent` (crouch held) and `game::movement::CaMovement` (moving, the
//! crouch blend, stamina); the held weapon's spread fraction r
//! (`game::weapons::CaSpread::fraction`; without it, r read back from
//! `Hitscan::spread`, the cone in perturb units / 1000), its data's
//! `combat_arms:spread` limits (`cone`, `zoomed_cone`, `burst_cone`) and
//! `combat_arms:hud` fields (`game::hud::CaHudData`), `Magazine`,
//! `AltModes`/`Burst`, `game::inventory::CaItem`, the shop item table's
//! names (`ItemNames`); `WeaponEvent`s: `Fired` (the kick; any player's
//! first shot, swing or throw shuts its spawn window), and the local
//! player's own `Shot` traces (a gun's first segment) and melee `Swing`s
//! on another character (the hit indicator and the name-tag flash, A10:
//! the shooter's own hit test on the frame of the shot, an enemy outside
//! its spawn window as this client sees it; on a network client its
//! predicted rounds' traces, nothing waiting for the server; grenades,
//! fire and poison never); `core::DamageTaken` (damage that reached
//! health, C: on a network client the server's report; refused hits and
//! hits under `god` show nothing), `core::Died` (the kill
//! feed), the match's phase (`game::rules::CaMatchView`: a round's end
//! clears the hit indicator, a round going live the damage overlay; also
//! `RoundRestarts`); bodies (`game::characters::CaBody`: a dead player's
//! tag) and what players carry (`CaCarry`); the aim ray (`client::aim`);
//! the pickup prompt's target (`CaSelection::pickup_target`, on a network
//! client the server's `game::net::CaPickup`).
//!
//! Known shortcuts (docs/tech-debt.md; the spec's most literal reading
//! where it is unclear):
//! - The HUD runs once per rendered frame on two clocks (Open question
//!   13): real time in whole ms (`Time<Real>`) for the hit indicator, the
//!   spawn windows, the damage overlay, the heartbeat, the action message
//!   and the kill feed, the game's time in seconds (`SimClock`) for the
//!   kick and the action gauge; Fired events are timed at the frame that
//!   reads them.
//! - "Enemy" for the hit indicator and name tags: a team id that differs
//!   from the local player's (the spec's, Open question 15); with no
//!   teams (the skeleton) everyone else, as if each player had an id of
//!   their own (which ids free-for-all modes give: a live check). A
//!   player's spawn window opens when this client first sees it alive or
//!   sees it alive again after death; a hit on a character counts, the
//!   hit box being the firing's (`game::weapons`).
//! - Spec yaw grows turning right: ours is `-Intent::yaw` (Q4).
//! - Name tag opacity (255 seen, 125 blocked) multiplies the name
//!   colour's own alpha (200); the tag is anchored at the skeleton's
//!   `NAME_socket` (`characters::CaPose`: a body model's, else the spec's
//!   "stand" pose, characters.md 3), at the character's position (the
//!   hull centre) without one; a dead player's at its latest body's
//!   resting place (our bodies have no pose, so no name socket), while
//!   the body exists; no distance limit, hidden only behind the camera
//!   (Q10). The rank, flag and revenge icons are placeholder marks (no
//!   rank or revenge data drives them yet: the flag icon only); Spy
//!   Hunt's marked-players rule for an intel holder isn't done.
//! - Text (Q2 and Q3, settled by ui_system.md): placed by `ui::layout`
//!   (its T2-T6): fonts of the font list (`data/ui/fonts.toml`) scaled
//!   ⌊H_f y_r⌋ tall and ⌊a_f x_r⌋ a glyph; aligned texts in their box,
//!   their own position ignored on the aligned axis (the weapon name
//!   right-aligned in its 250 x 10 box and centred on its y, the values
//!   centred, labels and ammo at their positions); outlines and shadows
//!   1 px at every size; the kill feed's names advance by their reference
//!   width ⌊w_px / x_r⌋ (`feed_width`). Glyph widths are our monospaced
//!   estimate (ui_system.md's Open question 3), drawn with the install's
//!   faces or stand-ins (`ui::draw`). The range readout's `SysFont13` at
//!   20 px is the install's face (`model::RANGE_FONT`; ours "bold" without
//!   it).
//! - The weapon panel's class is the inventory class (the data's `slot`,
//!   Q16); the fire-mode icon's `RapidFireCount` comes from the trigger
//!   (`rapid_fire_count`); continuous fire isn't done, so never on. The
//!   install's weapon names fill `ItemNames` (`install::weapon_names`, the
//!   item file's `ItemName` through the string table); a weapon without
//!   one shows the weapon data's name, else its id as words (`plain_name`).
//! - The held weapon's attachments (`game::attachments::CaMods`): the
//!   sight's range readout (A9; the local player's view ray, not a
//!   spectated player's) and the gauge sight's images (Corrections 13:
//!   with the weapon panel; the install's images beside the centre at our
//!   places, the special-weapon HUD layout's `OptionCrossHair` isn't
//!   read; without the install their numbers), the crosshair's
//!   attachment terms (A3).
//! - The kill feed: the revenge, gold, ice-crush and VIP badges aren't
//!   driven (no data); the groin badge is a hit in the stomach group
//!   (the firing's groin sphere); the flag badge reads what the victim
//!   carries when the HUD reads the kill (gone if the rules took the flag
//!   first); no damage kind of ours is poison or bleeding.
//! - Not driven by anything yet (the model has them): crosshair items,
//!   video and AI freeze, infection, dot-sight mode (A6: no sight has
//!   `Type` 2), revealing items, the state viewer, the pickup prompt's
//!   non-human, buff, VIP and Spy Hunt conditions. The heartbeat is
//!   counted in `CaHud::heartbeats` and sounds by `heartbeat_sound`. The
//!   crosshair is never toggled off (Q12): `HudOptions::crosshair_on` is a test hook.
//! - The score panel, round and bomb timers and the action gauge (D6, D7,
//!   D9) are `score`'s, from the match (`game::rules::CaMatchView`); see
//!   its own shortcuts; the bomb prompts `action`'s.
//! - Without the install (no V1 key), placeholders for the game's
//!   textures: the silhouette (a grey box standing or crouched), the
//!   fire-mode icon (a word), kill-feed weapon icons and badges (words),
//!   the hit overlay (a red frame), the direction indicator (a red bar
//!   along the square's top edge), the dot-sight image (a red outline, only when the install lacks the
//!   named file),
//!   the panels' backgrounds (dark boxes), the action message at (312,
//!   430) 400 x 20 (the game's `Action` panel places it). With it, the
//!   direction indicator is drawn as a rotated rectangle of its sides'
//!   lengths (no shear on non-4:3 screens) with the rows past the
//!   texture's last left empty (C's v overrun, no clamp addressing); the
//!   crosshair stays lines (A5); the weapon's dot-sight image (`DotSightName`: the knives' and
//!   robots' reticles, A8) is the install's own texture over the A8 rect; name tags stay ours (the spec
//!   gives no layout for them).

pub mod action;
pub mod bound;
pub mod death;
pub mod full_map;
pub mod guides;
pub mod install;
pub mod model;
pub mod radar;
pub mod scene;
pub mod score;

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::{prelude::*, ui::UiTransform, window::PrimaryWindow};
use serde::Deserialize;

use crate::{
    client::{
        FirstPersonCamera, aim,
        fonts::UiFonts,
        hud::{GameDraws, HudDraws, HudRoot},
        spectate::SpecView,
    },
    core::{
        Damage, DamageKind, DamageTaken, Dead, Died, Health, Hitgroup, Intent, LocalPlayer, MovementState,
        RoundRestarts, SimClock, Team,
    },
    game::{
        attachments::{CaMods, CaNpc, gauge_image},
        characters::CaBody,
        hud::CaHudData,
        rules::{CaMatchView, Phase, objectives::CaCarry},
        inventory::{self, CaItem, CaSelection, Dropped},
        movement::{CaCharacter, CaMovement, movement_data},
        net::CaPickup,
        weapons::CaSpread,
    },
    map::interp::RenderedView,
    net::{NetCharacter, NetSettings},
    weapon::{
        AltModes, Armor, Burst, Hitscan, Inventory, Magazine, SpreadShape, Trigger, Weapon, WeaponEvent, WeaponEventKind,
        Zoomed, grenade::Throwable,
    },
};
use model::*;

use super::{
    CaUi,
    draw::{self, CaFaces},
    layout::{self, Effect, FontList, REFERENCE, TextLook, text},
};
use crate::client::{
    canvas::{HAlign, Scale, VAlign},
    minigame_scoreboard::{GunNames, gun_name},
    weapon_icons::{WeaponIcon, WeaponIcons, spawn_weapon_icon},
};

/// The kill feed's names' font (D5).
pub const FEED_FONT: &str = "bold 12";

/// Metres per world unit (the aim ray's reach).
const UNIT: f32 = inventory::UNIT;
/// The model's InterfaceMaxHealth (standard soldiers).
const INTERFACE_MAX_HEALTH: f32 = 100.0;

// ---------------------------------------------------------------------------
// State

/// The screen the HUD lays out on, px: the window's (UI) size in the
/// game; tests set it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct HudScreen(pub Screen);

/// What the spec has that nothing in the game drives yet: crosshair
/// items, the crosshair toggle, infection. Tests set them.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HudOptions {
    /// The crosshair toggled on (A7; starts on).
    pub crosshair_on: bool,
    /// An equipped crosshair item's shape and colour (A6), and whether
    /// the room allows it.
    pub item_shape: Option<u8>,
    pub item_colour: Option<Rgba>,
    pub item_allowed: bool,
    pub infected: bool,
}

impl Default for HudOptions {
    fn default() -> Self {
        Self {
            crosshair_on: true,
            item_shape: None,
            item_colour: None,
            item_allowed: true,
            infected: false,
        }
    }
}

/// The crosshair this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrosshairView {
    pub draws: Draws,
    pub shape: u8,
    /// The limits and r the gap came from.
    pub limits: (i32, i32),
    pub r: f32,
    pub measures: Option<Measures>,
    /// What is drawn (empty when `draws.lines` is off).
    pub elements: Vec<Element>,
    pub colour: Rgba,
    pub dot_sight: Option<PxRect>,
    /// The reticle's texture (`DotSightName`, A8); the install's art draws it.
    pub dot_sight_image: Option<String>,
    /// The sight's range readout (A9): text and colour.
    pub range: Option<(String, Rgba)>,
    /// The gauge sight's images (attachments.md, "Crosshair, HUD and
    /// overlay"; A12, Corrections 13): health on the left, clip on the
    /// right, each 0..100 in tens; shown while the weapon panel shows.
    pub gauge: Option<(i32, i32)>,
    /// Their textures (the HUD layout attributes' prefixes with the
    /// steps), with the install's data.
    pub gauge_images: Option<(String, String)>,
}

/// The damage-taken pieces this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageView {
    pub alpha: u8,
    pub overlay: PxRect,
    pub corners: [Vec2; 4],
}

/// The health, armour and stamina block.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusView {
    pub hp: String,
    pub ap: String,
    pub low: bool,
    pub colours: HealthColours,
    pub crouched: bool,
    /// The stamina bar's width, ref px (0: hidden).
    pub stamina_bar: f32,
}

/// A name tag this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct TagView {
    pub entity: Entity,
    pub name: String,
    pub tag: Tag,
    /// Where it floats (world, metres).
    pub anchor: Vec3,
    /// Health as a fraction, for the HP bar.
    pub health: f32,
}

/// A kill-feed line: its top (ref) and its elements.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedLineView {
    pub y: f32,
    pub kill: Kill,
    pub items: Vec<FeedItem>,
}

/// A playing minigame's state as the HUD words it: the minigame's name,
/// its status lines (top to bottom) and its callout showing
/// (`MinigameHud::callout`, the first line big; `minigame_view`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MinigameView {
    pub title: String,
    pub lines: Vec<String>,
    pub callout: Vec<String>,
    /// The finished match's results (the winner, then a line a player).
    pub results: Vec<String>,
}

/// What Combat Arms' HUD shows this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaHud {
    pub screen: Screen,
    pub crosshair: CrosshairView,
    pub damage: Option<DamageView>,
    pub status: Option<StatusView>,
    pub weapon: Option<WeaponPanel>,
    pub feed: Vec<FeedLineView>,
    /// The playing minigame's state (its rung, the leader, its
    /// callout), only while one plays.
    pub minigame: Option<MinigameView>,
    /// The weapon mode switch's centre notice ("Switched to Burst-Fire
    /// mode", `weapon::notice::CenterNotice`).
    pub mode_notice: Option<String>,
    pub tags: Vec<TagView>,
    /// The local player's aim target (A11).
    pub aim_target: Option<Entity>,
    /// The action message (D9): the pickup, plant or defuse prompt.
    pub action: Option<ActionView>,
    /// Heartbeats played so far (D2).
    pub heartbeats: u32,
    /// The infected's screen texture's rect while the local player is
    /// infected (game_modes.md Corrections (2026-10-30) 3: the HUD
    /// layout's `InfectedTex` at `InfectedPos`, `InfectedSize`).
    pub infected: Option<PxRect>,
}

/// The action message as shown (D9): its string-table id and its text.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionView {
    pub id: String,
    pub text: String,
}

/// The shop item table's names by weapon id (D4, Corrections 16): the
/// weapon panel's name; a weapon without one shows its data's `hud.name`
/// (ours copy the item names), else its id.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ItemNames(pub HashMap<String, String>);

/// The HUD's memory between frames.
#[derive(Resource, Default, Debug)]
pub struct HudMemory {
    pub hit: HitIndicator,
    pub damage: Option<DamageFeedback>,
    pub heartbeat: Heartbeat,
    pub feed: KillFeed,
    /// Weapon: when it last fired, and last fired its alternate mode (s).
    pub fired: HashMap<Entity, (Option<f64>, Option<f64>)>,
    /// Who the local player last hit, and when (s): the name-tag flash.
    pub flashes: HashMap<Entity, f64>,
    pub colour: Option<Rgba>,
    pub rounds: Option<u32>,
    /// The match's phase last frame.
    pub phase: Option<Phase>,
    /// Each character: alive last frame, and its spawn window (A10).
    pub windows: HashMap<Entity, (bool, SpawnWindow)>,
    /// The action message (D9).
    pub message: ActionMessage,
}

/// `CaHudStatePlugin`'s systems.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HudUpdate;

/// The HUD's behaviour without a window (the scenario tests').
pub struct CaHudStatePlugin;

impl Plugin for CaHudStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudScreen>()
            .init_resource::<CaUi>()
            .init_resource::<HudOptions>()
            .init_resource::<HudMemory>()
            .init_resource::<CaHud>()
            .init_resource::<score::CaScoreHud>()
            .init_resource::<score::ScoreMemory>()
            .init_resource::<ItemNames>()
            .init_resource::<crate::game::rules::CaMatchView>()
            .add_message::<WeaponEvent>()
            .add_message::<Damage>()
            .add_message::<DamageTaken>()
            .add_message::<Died>()
            .add_message::<crate::game::effects::sounds::CaSound>()
            .add_plugins(radar::CaRadarStatePlugin)
            .add_systems(
                Update,
                (track, update, heartbeat_sound, action::prompts, minigame_view, mode_notice_view, score::update_score, hide_panels)
                    .chain()
                    .in_set(HudUpdate),
            );
    }
}

/// The low-health beat's sound (hud_crosshair.md D2; effects_sounds.md
/// Corrections 2026-10-22, 8): a file of the heartbeat entry, local, for
/// each beat the HUD counted.
fn heartbeat_sound(
    hud: Res<CaHud>,
    tables: Option<Res<crate::game::effects::data::CaFxTables>>,
    mut played: Local<u32>,
    mut rolls: Local<crate::game::effects::Rolls>,
    mut sounds: MessageWriter<crate::game::effects::sounds::CaSound>,
) {
    let beats = hud.heartbeats;
    // The count restarts with a new HUD (a session); only new beats sound.
    for _ in (*played).min(beats)..beats {
        if let Some(t) = &tables
            && let Some(f) = crate::game::effects::rules::draw_any(&t.sounds.heartbeat, rolls.next())
        {
            sounds.write(crate::game::effects::sounds::CaSound::local(f));
        }
    }
    *played = beats;
}

/// The HUD on/off key (options.md H): off hides every layout-built panel
/// (health, ammo, the kill feed, the prompt, the score panels); the
/// code-drawn items (crosshair, damage overlays, name tags) stay.
fn hide_panels(
    switch: Option<Res<crate::game::options::CaHudSwitch>>,
    mut hud: ResMut<CaHud>,
    mut score: ResMut<score::CaScoreHud>,
) {
    if switch.is_none_or(|s| s.panels()) {
        return;
    }
    if hud.status.is_some()
        || hud.weapon.is_some()
        || !hud.feed.is_empty()
        || hud.action.is_some()
        || hud.minigame.is_some()
    {
        hud.minigame = None;
        hud.status = None;
        hud.weapon = None;
        hud.feed.clear();
        hud.action = None;
    }
    if *score != score::CaScoreHud::default() {
        *score = score::CaScoreHud::default();
    }
}

/// The player the HUD is about: whom the local player watches in first
/// person, else the local player.
fn viewed(spec: Option<&SpecView>, local: Option<Entity>) -> Option<Entity> {
    spec.and_then(|s| s.in_eye).or(local)
}

fn enemies(a: Option<&Team>, b: Option<&Team>) -> bool {
    !matches!((a, b), (Some(a), Some(b)) if a == b)
}

/// What the kill feed calls character `e`: the shell's `shown_name` (the
/// server's name in a network game, the `name` cvar for the local player,
/// a bot's own name), never the character's default entity name.
fn name_of(names: &Query<NameParts>, settings: Option<&NetSettings>, e: Entity) -> String {
    match names.get(e) {
        // A name the character was given (a test's, a game's) stands; the
        // default entity name ("Character") is no player's.
        Ok((Some(name), true, None)) if name.as_str() != "Character" => name.to_string(),
        Ok((name, local, net)) => crate::client::shown_name(e, local, net, name, settings),
        Err(_) => "Player".to_string(),
    }
}

/// What the HUD calls weapon `id`: the item table's name (an app may add
/// another game's guns' names to it: CS:S's from its strings), else its
/// data's `hud.name`, else its id.
pub(crate) fn weapon_name(items: &ItemNames, catalog: &crate::weapon::data::WeaponCatalog, id: &str) -> String {
    items.0.get(id).cloned().unwrap_or_else(|| {
        catalog
            .get(id)
            .and_then(|d| d.hud.name.clone())
            .unwrap_or_else(|| plain_name(id))
    })
}

/// A weapon id as a readable name when nothing names it: its last part
/// in capitals, `_` and `-` as spaces (`combat_arms:beretta_m92fs` is
/// `BERETTA M92FS`).
pub fn plain_name(id: &str) -> String {
    id.rsplit(':')
        .next()
        .unwrap_or(id)
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c.to_ascii_uppercase() })
        .collect()
}

/// `CaHud::minigame` from the playing minigame's `MinigameHud`: its lines'
/// `{player}` and `{weapon}` filled in (the kill feed's names, the weapon
/// panel's names).
fn minigame_view(
    minigame: Option<Res<chimera_sim::minigame::hud::MinigameHud>>,
    items: Res<ItemNames>,
    catalog: Res<crate::weapon::data::WeaponCatalog>,
    (names, settings): (Query<NameParts>, Option<Res<NetSettings>>),
    mut hud: ResMut<CaHud>,
) {
    // The shell's match-end screen up: it has the results, and the panel
    // would draw into it.
    let Some(m) = minigame.filter(|m| m.playing() && !m.results_shown) else {
        hud.minigame = None;
        return;
    };
    let word = |l: &chimera_sim::minigame::hud::HudLine| {
        let mut text = l.text.clone();
        if let Some(p) = l.player {
            text = text.replace("{player}", &name_of(&names, settings.as_deref(), p));
        }
        if let Some(w) = l.weapon {
            text = text.replace("{weapon}", &weapon_name(&items, &catalog, w));
        }
        text
    };
    hud.minigame = Some(MinigameView {
        title: m.title.clone(),
        lines: m.lines.iter().map(word).collect(),
        callout: m.callout.as_ref().map_or_else(Vec::new, |c| c.callout.lines.iter().map(word).collect()),
        results: m.results.as_ref().map_or_else(Vec::new, |r| {
            r.lines(|row| row.player.map_or_else(|| row.name.clone(), |p| name_of(&names, settings.as_deref(), p)))
        }),
    });
}

/// `CaHud::mode_notice` from the shell's `CenterNotice`, while it lasts.
fn mode_notice_view(
    notice: Option<Res<crate::weapon::notice::CenterNotice>>,
    time: Res<Time>,
    mut hud: ResMut<CaHud>,
) {
    let text = notice.and_then(|n| n.text(time.elapsed_secs()).map(str::to_string));
    if hud.mode_notice != text {
        hud.mode_notice = text;
    }
}

/// The mode switch's notice under the middle of the view, in the notices'
/// looks (ours: the install's HUD has no such line).
fn draw_mode_notice(
    hud: Res<CaHud>,
    params: PenParams,
    root: Single<Entity, With<NoticeRoot>>,
    mut last: Local<Option<(Option<String>, Screen)>>,
    mut commands: Commands,
) {
    if !rebuild(&mut last, (hud.mode_notice.clone(), hud.screen), *root, &mut commands) {
        return;
    }
    let Some(text) = &hud.mode_notice else { return };
    let pen = pen(&params, hud.screen);
    commands.entity(*root).with_children(|p| {
        hud_text(
            p,
            pen,
            (Vec2::new(262.0, 300.0), Vec2::new(500.0, 28.0), Vec2::ZERO),
            text,
            looks::NOTICE,
            (HAlign::Centre, VAlign::Centre),
            Effect::Stroke,
            [255, 230, 120, 255],
        );
    });
}

/// What `name_of` reads of a character.
type NameParts<'a> = (Option<&'a Name>, Has<LocalPlayer>, Option<&'a NetCharacter>);

/// The HUD's two clocks (Units and conventions, "Time"): the OS
/// millisecond clock (real time, whole ms) for the hit indicator, the
/// damage overlay and the kill feed, and the game's time in seconds (the
/// simulation's, else the frame's) for the crosshair kick.
fn clocks(real: &Time<Real>, time: &Time, sim: Option<&SimClock>) -> (f64, f64) {
    let ms = real.elapsed().as_millis() as f64;
    (ms, sim.map_or_else(|| time.elapsed_secs_f64(), |c| c.now))
}

/// The events of this frame: shots (the kick, the hit indicator from the
/// local player's own traces, the spawn windows' first shots), damage
/// taken, kills, a round's end and a new round going live.
#[allow(clippy::too_many_arguments)]
fn track(
    (time, real, sim): (Res<Time>, Res<Time<Real>>, Option<Res<SimClock>>),
    mut memory: ResMut<HudMemory>,
    mut weapons: MessageReader<WeaponEvent>,
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    local: Option<Single<(Entity, &Intent, Option<&Team>), With<LocalPlayer>>>,
    characters: Query<(Entity, &Health, Option<&Team>, Has<Dead>), With<Intent>>,
    (names, settings): (Query<NameParts>, Option<Res<NetSettings>>),
    (held, ids, carry, npcs): (Query<&Inventory>, Query<&Weapon>, Query<&CaCarry>, Query<(), With<CaNpc>>),
    (rounds, view, shared): (
        Option<Res<RoundRestarts>>,
        Option<Res<CaMatchView>>,
        Option<Res<crate::client::kill_feed::SharedKillFeed>>,
    ),
) {
    let (ms, now) = clocks(&real, &time, sim.as_deref());
    let memory = &mut *memory;
    let (me, intent, my_team) = match local.as_deref() {
        Some(&(e, i, t)) => (Some(e), Some(i.clone()), t.copied()),
        None => (None, None, None),
    };
    // The spawn windows (A10): opened when a character appears or comes
    // back to life on this client.
    for (e, health, _, dead) in &characters {
        let alive = !dead && health.current > 0.0;
        let (was, window) = memory.windows.entry(e).or_insert((false, SpawnWindow::default()));
        if alive && !*was {
            window.spawn(ms);
        }
        *was = alive;
    }
    memory.windows.retain(|e, _| characters.contains(*e));
    let team_of = |e: Entity| characters.get(e).ok().and_then(|c| c.2.copied());
    // Shots: the kick; a player's first shot shuts its spawn window; the
    // local player's own traces (a gun's first segment, a melee swing's
    // line) on another character light the hit indicator (Corrections 1:
    // nothing waits for the server, grenades and burns never light it).
    let mut traced: Vec<Entity> = Vec::new();
    for e in weapons.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. } | WeaponEventKind::Swing { .. } | WeaponEventKind::Thrown)
            && let Some((_, w)) = memory.windows.get_mut(&e.owner)
        {
            w.fire();
        }
        if !e.shown() {
            continue;
        }
        match e.kind {
            WeaponEventKind::Fired { mode, .. } => {
                let entry = memory.fired.entry(e.weapon).or_default();
                entry.0 = Some(now);
                if mode != 0 {
                    entry.1 = Some(now);
                }
            }
            WeaponEventKind::Shot { hit: Some(target), .. } if Some(e.owner) == me => traced.push(target),
            WeaponEventKind::Swing { at: Some((_, _, target)), .. } if Some(e.owner) == me => traced.push(target),
            _ => {}
        }
    }
    for target in traced {
        if Some(target) == me || !characters.contains(target) {
            continue;
        }
        let enemy = enemies(my_team.as_ref(), team_of(target).as_ref());
        let open = memory.windows.get(&target).is_some_and(|(_, w)| w.open(ms));
        if hit_lights(enemy, open) {
            memory.hit.hit(ms, true);
            memory.flashes.insert(target, now);
        }
    }
    // Damage taken (C): what reached health (`DamageTaken`: the server's
    // report on a network client, `apply_damage`'s here). A hit refused
    // or taken under `god` loses no health and shows nothing.
    let hits: Vec<Damage> = taken.read().map(|t| t.damage.clone()).collect();
    if let (Some(me), Some(intent)) = (me, intent) {
        for d in hits.iter().filter(|d| d.target == me && d.amount > 0.0) {
            let (f, rt) = view_axes(&intent);
            // No direction: the world's forward axis (Corrections 2).
            let dir = if d.dir.length_squared() > 0.0 { d.dir } else { NO_DIRECTION };
            memory.damage = DamageFeedback::hit(ms, d.amount, f, rt, dir, -intent.yaw).or(memory.damage);
        }
    }
    // The overlay fades on every frame, its hit's own included
    // (Corrections 3: the first frame draws 253).
    if let Some(d) = memory.damage.as_mut()
        && !d.frame(ms)
    {
        memory.damage = None;
    }
    // The kill feed (D5, Corrections 7): a self-kill names its killer.
    // None while the app draws one feed for every HUD (`client::kill_feed`).
    if shared.is_some() {
        died.clear();
        memory.feed.lines.clear();
    }
    for d in died.read() {
        let mate = |e: Entity| Some(e) == me || !enemies(my_team.as_ref(), team_of(e).as_ref());
        let killer = d.attacker;
        let kind = match d.damage.kind {
            DamageKind::Crush | DamageKind::Fall => DeathKind::Crush,
            DamageKind::Blast => DeathKind::Blast,
            DamageKind::Burn => DeathKind::Burn,
            _ => DeathKind::Other,
        };
        let weapon = d.damage.weapon.map(str::to_string).or_else(|| {
            let k = killer.filter(|k| *k != d.entity)?;
            let w = held.get(k).ok()?.active?;
            Some(ids.get(w).ok()?.id.to_string())
        });
        let npc = killer.is_some_and(|k| npcs.contains(k));
        let flag = carry.get(d.entity).is_ok_and(|c| c.flag.is_some());
        let badges = kill_badges(
            false,
            d.damage.hitgroup == Hitgroup::Head,
            d.damage.hitgroup == Hitgroup::Stomach,
            flag,
            mate(d.entity),
        );
        memory.feed.push(
            ms,
            Kill {
                killer: killer.map(|k| (name_of(&names, settings.as_deref(), k), mate(k))),
                victim: (name_of(&names, settings.as_deref(), d.entity), mate(d.entity)),
                icon: kill_icon(killer.is_some(), npc, kind, weapon),
                badges,
            },
        );
    }
    memory.feed.frame(ms);
    // A round's end clears the hit indicator (A10); a round going live
    // the damage overlay (C).
    let round = rounds.map(|r| r.0);
    if memory.rounds.is_some() && memory.rounds != round {
        memory.hit.reset();
        memory.damage = None;
    }
    memory.rounds = round;
    let phase = view.map(|v| v.phase);
    if phase != memory.phase {
        match phase {
            Some(Phase::RoundOver { .. }) => memory.hit.reset(),
            Some(Phase::Live) => memory.damage = None,
            _ => {}
        }
        memory.phase = phase;
    }
    memory.flashes.retain(|_, t| now - *t < TAG_FLASH_S);
}

/// The damage report's direction for damage with none (C, Corrections
/// 2): the world's forward axis, the way a yaw-0 player faces.
pub const NO_DIRECTION: Vec3 = Vec3::NEG_Z;

/// The forward of a player's view rotation (pitch and yaw: it tilts with
/// the pitch) and its horizontal right (C).
pub fn view_axes(intent: &Intent) -> (Vec3, Vec3) {
    (intent.look_rotation() * Vec3::NEG_Z, intent.yaw_rotation() * Vec3::X)
}

/// The `combat_arms:spread` limits the crosshair reads (the firing part's
/// data; other fields ignored).
#[derive(Clone, Copy, Debug, Default, Deserialize)]
struct Cone {
    #[serde(default)]
    min: i32,
    #[serde(default)]
    max: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
struct SpreadLimits {
    #[serde(default)]
    cone: Cone,
    #[serde(default)]
    zoomed_cone: Option<Cone>,
    #[serde(default)]
    burst_cone: Option<Cone>,
}

/// A kill-feed name's width in ref px (Q2, ui_system.md T3): measured in
/// screen px with its font scaled to the screen, ⌊w_px / x_r⌋.
pub fn feed_width(fonts: &FontList, screen: Screen, s: &str) -> f32 {
    fonts
        .get(FEED_FONT)
        .map_or(0.0, |f| text::measure(s, f, 0, scale(screen)).reference.x as f32)
}

/// The HUD's screen scale (x_r, y_r).
pub fn scale(screen: Screen) -> Scale {
    Scale::of(Vec2::new(screen.w, screen.h), REFERENCE)
}

/// The firing part's spread hook (weapon_firing.md).
pub const SPREAD_HOOK: &str = "combat_arms:spread";

/// The crosshair's perturb data and r for a weapon whose cone is `n`
/// perturb units, from its data's spread limits (`limits`, the
/// `combat_arms:spread` table) and its HUD data, in `stance`.
pub fn spread_of(limits: Option<&toml::Value>, hud: &CaHudData, n: f32, stance: Stance) -> (Perturb, f32) {
    let Some(l) = limits.and_then(|v| v.clone().try_into::<SpreadLimits>().ok()) else {
        let n = n.round() as i32;
        let p = Perturb {
            min: n,
            max: n,
            ..default()
        };
        return (p, 0.0);
    };
    let z = l.zoomed_cone.unwrap_or_default();
    let b = l.burst_cone.unwrap_or_default();
    // The firing part's own limits in this mode (its non-zero zoomed and
    // burst values), to read r back from the cone.
    let pick = |v: i32, base: i32| if v != 0 { v } else { base };
    let (lo, hi) = if stance.zoomed && l.zoomed_cone.is_some() {
        (pick(z.min, l.cone.min), pick(z.max, l.cone.max))
    } else if stance.burst && l.burst_cone.is_some() {
        (pick(b.min, l.cone.min), pick(b.max, l.cone.max))
    } else {
        (l.cone.min, l.cone.max)
    };
    let r = if hi > lo { ((n - lo as f32) / (hi - lo) as f32).max(0.0) } else { 0.0 };
    let p = Perturb {
        min: l.cone.min,
        max: l.cone.max,
        zoomed_min: z.min,
        zoomed_max: z.max,
        burst_min: b.min,
        burst_max: b.max,
        duck: hud.duck_perturb,
        move_duck: hud.move_duck_perturb,
    };
    (p, r)
}

/// A weapon's `RapidFireCount` from its trigger: 0 fully automatic, 1
/// semi-automatic, n stopping after n (none: 1).
pub fn rapid_fire_count(trigger: Option<&Trigger>) -> u32 {
    match trigger {
        Some(t) if t.per_press >= 2 => t.per_press,
        Some(t) if t.automatic => 0,
        _ => 1,
    }
}

type Viewed<'a> = (
    &'a Health,
    Option<&'a Inventory>,
    Option<&'a Zoomed>,
    Option<&'a Intent>,
    Option<&'a CaMovement>,
    Option<&'a CaCharacter>,
    Option<&'a Armor>,
    Has<Dead>,
);

type HeldWeapon<'a> = (
    &'a Weapon,
    Option<&'a Hitscan>,
    Option<&'a CaHudData>,
    Option<&'a AltModes>,
    Option<&'a Burst>,
    Option<&'a Magazine>,
    Option<&'a CaItem>,
    Option<&'a CaSpread>,
    (Option<&'a Trigger>, Option<&'a Throwable>, Option<&'a CaMods>),
);

type Character<'a> = (
    Entity,
    &'a Transform,
    &'a Health,
    Option<&'a Team>,
    Option<&'a Name>,
    Has<Dead>,
    Option<&'a ColliderAabb>,
);

#[derive(bevy::ecs::system::SystemParam)]
struct HudInputs<'w, 's> {
    local: Option<
        Single<
            'w,
            's,
            (
                Entity,
                &'static Transform,
                &'static Intent,
                &'static MovementState,
                Option<&'static RenderedView>,
                Option<&'static Team>,
                Option<&'static CaSelection>,
                Option<&'static CaPickup>,
            ),
            With<LocalPlayer>,
        >,
    >,
    spec: Option<Res<'w, SpecView>>,
    viewed: Query<'w, 's, Viewed<'static>>,
    weapons: Query<'w, 's, HeldWeapon<'static>>,
    characters: Query<'w, 's, Character<'static>, With<Intent>>,
    parents: Query<'w, 's, &'static ChildOf>,
    dropped: Query<'w, 's, &'static Dropped>,
    spatial: SpatialQuery<'w, 's>,
    /// Name sockets (characters.md 3); a character of another game's
    /// identity wears that game's body (its hitboxes' top).
    poses: Query<
        'w,
        's,
        (
            Option<&'static crate::game::characters::CaPose>,
            &'static crate::core::MovementState,
            Option<&'static crate::slots::GameIdentity>,
            Option<&'static crate::core::Hitboxes>,
        ),
    >,
    /// Dead players' bodies (their tags float over them) and what players
    /// carry (a tag's flag icon).
    bodies: Query<'w, 's, &'static CaBody>,
    carry: Query<'w, 's, &'static CaCarry>,
    items: Res<'w, ItemNames>,
    /// The death camera: whom the dead player watches.
    death: Option<Res<'w, crate::game::death_cam::CaDeathCam>>,
    /// The weapons this app loaded.
    catalog: Res<'w, crate::weapon::data::WeaponCatalog>,
    /// The infected (a Quarantine round's, a ladder rung's).
    infected: Query<'w, 's, (), Or<(With<crate::game::movement::CaInfected>, With<crate::game::rungs::CaRungInfected>)>>,
}

/// How far over another game's body's head (its hitboxes' top) its name
/// tag floats, m (ours: the skeleton's name socket).
const NAME_ABOVE_HEAD: f32 = 0.15;

/// This frame's `CaHud` from the world and the memory.
fn update(
    (time, real, sim): (Res<Time>, Res<Time<Real>>, Option<Res<SimClock>>),
    screen: Res<HudScreen>,
    options: Res<HudOptions>,
    ui: Res<CaUi>,
    mut memory: ResMut<HudMemory>,
    world: HudInputs,
    mut hud: ResMut<CaHud>,
) {
    let (ms, now) = clocks(&real, &time, sim.as_deref());
    let screen = screen.0;
    let memory = &mut *memory;
    let local = world.local.as_deref().copied();
    let in_eye = world.spec.as_deref().and_then(|s| s.in_eye);
    let who = viewed(world.spec.as_deref(), local.map(|l| l.0));
    let player = who.and_then(|e| world.viewed.get(e).ok());
    let hit = memory.hit.frame(ms);
    let mut next = CaHud {
        screen,
        heartbeats: hud.heartbeats,
        // `action::prompts`' (after this).
        action: hud.action.clone(),
        ..default()
    };

    // The infected's screen, drawn first by the crosshair element, before
    // its other tests (game_modes.md Corrections (2026-10-30) 3).
    if options.crosshair_on && local.is_some_and(|l| world.infected.contains(l.0)) {
        let point = |key: &str, default: [i32; 2]| {
            ui.install.as_ref().and_then(|i| i.hud_point(key)).unwrap_or(default)
        };
        next.infected = Some(model::infected_rect(screen, point("InfectedPos", [0, 0]), point("InfectedSize", [1024, 768])));
    }

    // The crosshair (A).
    let active = player.and_then(|p| p.1).and_then(|i| i.active);
    let weapon = active.and_then(|w| world.weapons.get(w).ok().map(|x| (w, x)));
    let dead = player.is_none_or(|p| p.7 || p.0.current <= 0.0);
    let scope = player.and_then(|p| p.2).is_some_and(|z| z.scope);
    let default_hud = CaHudData::default();
    let data = weapon.and_then(|(_, w)| w.2).unwrap_or(&default_hud);
    // The held weapon's attachments (its sight's readout and gauge).
    let mods = weapon.and_then(|(_, w)| w.8.2);
    let visibility = model::Visibility {
        ok: options.crosshair_on,
        scope_overlay: scope,
        death_cam: dead && in_eye.is_none(),
        weapon: weapon.is_some(),
        disabled: data.disable_crosshair,
        dot_sight: data.dot_sight.is_some(),
        range_readout: mods.is_some_and(|m| m.range_readout().is_some()),
    };
    let draws = model::draws(visibility);
    let colour = choose_colour(
        ColourInputs {
            hit,
            dot_sight: None,
            item: options.item_colour,
            weapon: weapon.map(|_| data.crosshair_color),
        },
        memory.colour.unwrap_or(DEFAULT_CROSSHAIR),
    );
    memory.colour = Some(colour);
    next.crosshair = CrosshairView {
        draws,
        colour,
        dot_sight: (draws.dot_sight > 0).then(|| dot_sight_rect(screen)),
        dot_sight_image: data.dot_sight.clone(),
        ..default()
    };
    if let (Some((w, (id, scan, _, modes, burst, _, _, spread, _))), Some(p)) = (weapon, player) {
        let n = scan.map_or(0.0, |s| match s.spread {
            SpreadShape::Template(s) => s,
            SpreadShape::Disc { inaccuracy, spread } => inaccuracy + spread,
        }) * 1000.0;
        let bursting = matches!((modes, burst), (Some(m), Some(b)) if b.mode == m.current && b.count > 0);
        let stance = Stance {
            zoomed: p.2.is_some_and(|z| !z.scope),
            burst: bursting,
            crouch: p.3.is_some_and(|i| i.crouch),
            moving: p.4.is_some_and(|m| m.moving),
        };
        let limits = world.catalog.get(id.id).and_then(|d| d.hooks.get(SPREAD_HOOK));
        let (perturb, r) = spread_of(limits, data, n, stance);
        // The firing part's own fraction when it runs on this weapon.
        let r = spread.map_or(r, CaSpread::fraction);
        let perturb_mods = PerturbMods {
            attachments: mods.map(CaMods::crosshair_terms).unwrap_or_default(),
            ..default()
        };
        let (min, max) = perturb_limits(&perturb, &perturb_mods, stance);
        let (fired, alt) = memory.fired.get(&w).copied().unwrap_or_default();
        let since = |t: Option<f64>| t.map(|t| (now - t) as f32);
        let shape = choose_shape(data.crosshair, options.item_shape, options.item_allowed);
        let (m, elements) = crosshair_elements(
            &CrosshairInputs {
                weapon_shape: data.crosshair,
                shape,
                min,
                max,
                r,
                since_fire: since(fired),
                since_alt: since(alt),
                offset: data.crosshair_offset,
                wide: data.wide_count,
            },
            screen,
        );
        let c = &mut next.crosshair;
        c.shape = shape;
        c.limits = (min, max);
        c.r = r;
        c.measures = Some(m);
        if draws.lines {
            c.elements = elements;
        }
    }

    // The sight (attachments.md, "Crosshair, HUD and overlay"): the range
    // readout to what the local player's view ray hits within the sight's
    // reach (A9), the gauge of health and clip.
    if let (Some(m), Some((_, w))) = (mods, weapon) {
        if draws.range_readout
            && let Some(reach) = m.range_readout()
            && let Some((me, at, intent, state, view, ..)) = local
            && who == Some(me)
        {
            let ray = aim::view_ray(at, intent, state, view);
            let filter = SpatialQueryFilter::from_excluded_entities([me]).with_mask(crate::core::SOLID_LAYERS);
            let hit = world
                .spatial
                .cast_ray(ray.eye, ray.dir, reach as f32 * UNIT, true, &filter)
                .map(|h| h.distance / UNIT);
            let range = w.1.map_or(0.0, |s| (s.range / UNIT).round());
            next.crosshair.range = Some(range_readout(hit, range, m.damage().range_per));
        }
    }

    // Damage taken (C): drawn whenever the crosshair item draws.
    if draws.damage
        && let Some(d) = memory.damage
    {
        let yaw = local.map_or(0.0, |l| -l.2.yaw);
        next.damage = Some(DamageView {
            alpha: d.alpha,
            overlay: hit_overlay_rect(screen),
            corners: d.corners(yaw, screen),
        });
    }

    // Health, armour, stamina (D2, D3): the local player's, alive and not
    // watching anyone.
    let status_shown = who.is_some() && who == local.map(|l| l.0) && !dead;
    if status_shown && let Some(p) = player {
        let (h, max) = (p.0.current * 100.0, p.0.max * 100.0);
        let low = low_health(h, max, options.infected);
        if memory.heartbeat.frame(ms, low) {
            next.heartbeats += 1;
        }
        let character = p.5.unwrap_or(&movement_data().character);
        next.status = Some(StatusView {
            hp: hp_shown(h, max, INTERFACE_MAX_HEALTH).to_string(),
            ap: ap_shown(p.6.map_or(0.0, |a| a.amount * 100.0)).to_string(),
            low,
            colours: health_colours(low),
            crouched: crouch_image(p.4.map_or(0.0, |m| m.crouch_rate)),
            stamina_bar: p.4.map_or(0.0, |m| stamina_bar(m.stamina, character.stamina_max())),
        });
    }

    // The weapon panel (D4).
    if let Some((_, (id, _, hud_data, modes, burst, mag, item, _, (trigger, throw, _)))) = weapon {
        let bursting = match (modes, burst) {
            (Some(m), Some(b)) if b.mode == m.current && b.count > 0 => Some(b.count),
            _ => None,
        };
        // A thrown weapon's count is its clip, nothing in reserve.
        let (clip, total) = match (mag, throw) {
            (Some(m), _) => (m.clip, m.clip + m.reserve),
            (None, Some(t)) => (t.count, t.count),
            _ => (0, 0),
        };
        let inputs = PanelInputs {
            name: weapon_name(&world.items, &world.catalog, id.id),
            class: inventory::class_of(id.slot),
            bomb: hud_data.is_some_and(|h| h.bomb),
            infinite: item.is_some_and(|i| i.infinite) || (mag.is_none() && throw.is_none()),
            clip,
            total,
            burst: bursting,
            rapid_fire_count: rapid_fire_count(trigger),
            // Continuous fire isn't done (weapon_firing.md): never on.
            continuous: false,
        };
        next.weapon = model::weapon_panel(&inputs, status_shown && !options.infected);
    }

    // The gauge sight's images (A12, Corrections 13): the weapon panel's,
    // whenever it shows, crosshair or not.
    if let (Some(m), Some((_, w)), Some(p), Some(_)) = (mods, weapon, player, &next.weapon)
        && m.gauge()
    {
        // The integer percentage (health is a fraction here: a hair of
        // slack so 0.95 is 95, not 94.99).
        let percent = |v: f32, max: f32| if max > 0.0 { (100.0 * v / max + 1e-4).floor() as i32 } else { 0 };
        let hp = percent(p.0.current, p.0.max);
        let clip = w.5.map_or(0, |mag| percent(mag.clip as f32, mag.size as f32));
        next.crosshair.gauge = Some((gauge_image(hp), gauge_image(clip)));
        let prefix = |k: &str| ui.install.as_ref().and_then(|i| i.hud_prefix(k));
        if let (Some(l), Some(r)) = (prefix("crosshairlefttex"), prefix("crosshairrighttex")) {
            next.crosshair.gauge_images = Some(gauge_images(&l, &r, hp, clip));
        }
    }

    // The kill feed (D5): shown even while dead.
    next.feed = memory
        .feed
        .lines
        .iter()
        .enumerate()
        .map(|(i, (_, k))| FeedLineView {
            y: KillFeed::line_y(i),
            kill: k.clone(),
            items: feed_line(k, |s| feed_width(&ui.fonts, screen, s)),
        })
        .collect();

    // Name tags and the aim target (A11).
    if let Some((me, at, intent, state, view, my_team, selection, served)) = local {
        let ray = aim::view_ray(at, intent, state, view);
        let local_alive = world.viewed.get(me).is_ok_and(|p| !p.7 && p.0.current > 0.0);
        let target = if local_alive {
            aim::character_under(
                &world.spatial,
                ray,
                AIM_RANGE * UNIT,
                me,
                |e| world.characters.contains(e),
                &world.parents,
            )
            .map(|(e, _)| e)
        } else {
            None
        };
        next.aim_target = target;
        // Each dead player's latest body (Corrections 5).
        let mut bodies: HashMap<Entity, &CaBody> = HashMap::new();
        for b in &world.bodies {
            if bodies.get(&b.owner).is_none_or(|old| old.since <= b.since) {
                bodies.insert(b.owner, b);
            }
        }
        for (e, t, health, team, name, their_dead, _) in &world.characters {
            if e == me {
                continue;
            }
            let alive = !their_dead && health.current > 0.0;
            // A dead player's tag floats over its body (our bodies have no
            // pose: their resting place); a living one's at the skeleton's
            // name socket, without one the character's position, the hull
            // centre (Open question 10). Wearing another game's body (its
            // identity's), over that body's head: its hitboxes' top, not
            // our skeleton's socket, which it doesn't show.
            let body = (!alive).then(|| bodies.get(&e)).flatten();
            let aabb = world.characters.get(e).ok().and_then(|c| c.6);
            let anchor = match body {
                Some(b) => b.at,
                None => world
                    .poses
                    .get(e)
                    .ok()
                    .and_then(|(p, s, identity, boxes)| {
                        if !crate::slots::GameIdentity::allows(identity, crate::game::GAME) {
                            let top = boxes?.bounds()?.1.y;
                            let feet = aabb.map_or(t.translation.y, |b| b.min.y);
                            return Some(t.translation.with_y(feet + top + NAME_ABOVE_HEAD));
                        }
                        let p = p?;
                        Some(p.world(t, s, UNIT, p.name_tag?))
                    })
                    .unwrap_or(t.translation),
            };
            let inputs = TagInputs {
                local_alive,
                teammate: !enemies(my_team, team),
                alive,
                aimed: target == Some(e),
                revealed: false,
                flashing: memory.flashes.contains_key(&e),
                seen: aim::clear_line(&world.spatial, ray.eye, anchor, &[me, e]),
                chased: in_eye == Some(e) || world.death.as_ref().is_some_and(|d| d.watched == Some(e)),
                everyone: !local_alive && world.death.as_ref().is_some_and(|d| d.rules.hunted),
                state_viewer: false,
                body_visible: body.is_some(),
                flag: world.carry.get(e).is_ok_and(|c| c.flag.is_some()),
                revenge: false,
            };
            if let Some(tag) = name_tag(inputs) {
                next.tags.push(TagView {
                    entity: e,
                    name: name.map_or_else(|| "Player".to_string(), |n| n.to_string()),
                    tag,
                    anchor,
                    health: if health.max > 0.0 { health.current / health.max } else { 0.0 },
                });
            }
        }
        // The pickup prompt (D9, Corrections 11): asked for from the
        // crosshair's draw step while the view is on a pickup (here, or on
        // a network client the server's), a fixed string-table text.
        let here = selection.and_then(|s| s.pickup_target).is_some_and(|i| world.dropped.contains(i));
        let prompt = pickup_prompt(
            PickupInputs {
                draw_step: options.crosshair_on && status_shown,
                scope_overlay: scope,
                death_cam: dead && in_eye.is_none(),
                weapon_data: weapon.is_some(),
                aiming_at_pickup: here || served.is_some_and(|p| p.0.is_some()),
                mine: data.mine,
                infected: options.infected,
                ..default()
            },
            false,
        );
        if let Some(id) = prompt {
            memory.message.request(id, ms);
        }
    }
    hud.set_if_neq(next);
}

// ---------------------------------------------------------------------------
// Drawing

/// Combat Arms' HUD, drawn: its behaviour (`CaHudStatePlugin`) and the
/// shell's UI nodes for it; the shell's fallback HUD leaves out what this
/// draws.
pub struct CaHudPlugin;

impl Plugin for CaHudPlugin {
    fn build(&self, app: &mut App) {
        // The install's UI data over ours (`ui::install`), and the
        // game's HUD panels bound by name when its HUD layout reads.
        let data = crate::game::data::CaData::of(app);
        let ui = app.world().get_resource::<CaUi>().cloned().unwrap_or_else(|| CaUi::installed(&data));
        let panels = ui.install.as_ref().and_then(|i| i.hud.as_ref()).and_then(install::HudPanels::new);
        let bound = BoundHud::of(&ui);
        // The HUD (its state's and radar's updates, `HudUpdate` and
        // `radar::RadarUpdate`, and the drawing below) runs while it is the
        // gameplay's (`gameplay::hud_is`); Combat Arms' is the gameplay's
        // when the app has chosen none.
        {
            use crate::gameplay::{Gameplay, GameplayAppExt};
            app.default_gameplay(Gameplay::of(&crate::game::INFO));
        }
        // The install's weapon names (the shop item file's) for the weapon
        // panel and the kill feed; an app adds other games' guns' later.
        let mut names = ItemNames::default();
        if let Some(i) = &ui.install {
            names.0.extend(i.weapon_names.iter().cloned());
        }
        // The install's attachment names: the gun game's rung line
        // (`rungs::label`).
        if let Some(i) = &ui.install {
            app.insert_resource(crate::game::rungs::CaPartNames::new(i.part_names.iter().cloned()));
        }
        app.insert_resource(ui)
            .insert_resource(names)
            .insert_resource(InstallHud(panels))
            .insert_resource(bound)
            .add_plugins(CaHudStatePlugin)
            .insert_resource(DRAWS);
        // `GameDraws` is ours while our HUD is the gameplay's (an app with
        // another game's HUD too: the shell's `HudDraws`).
        app.world_mut().get_resource_or_init::<HudDraws>().0.push((crate::game::GAME, DRAWS));
        // Other games' HUDs draw our guns with the install's art
        // (`WeaponIcons`), whichever HUD is up.
        app.init_resource::<WeaponIcons>().add_systems(Update, register_icons);
        app
            .add_systems(
                Startup,
                (spawn, score::spawn_root, draw::load_faces, draw::load_art, radar::spawn_root, full_map::spawn_root, guides::spawn_root),
            )
            .add_systems(Update, screen_size.before(HudUpdate).run_if(crate::gameplay::hud_is(crate::game::GAME)))
            .add_systems(
                Update,
                (
                    draw_crosshair,
                    draw_infected,
                    draw_damage,
                    draw_status,
                    draw_weapon,
                    draw_install_panels,
                    draw_action,
                    draw_feed,
                    draw_tags,
                    guides::draw_guides,
                    draw_minigame,
                    draw_mode_notice,
                    score::draw_score,
                )
                    .after(HudUpdate)
                    .distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
            )
            .add_systems(
                Update,
                (radar::draw_radar, full_map::draw_full_map)
                    .after(radar::RadarUpdate)
                    .distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
            )
            // After `CaHudStatePlugin` made the sets (configured earlier,
            // they would be new graph nodes ahead of others, which
            // reorders unordered systems).
            .configure_sets(
                Update,
                (HudUpdate, radar::RadarUpdate).distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
            );
    }
}

/// What this HUD draws instead of the shell's fallback HUD.
pub const DRAWS: GameDraws = GameDraws {
    crosshair: true,
    status: true,
    hit_marker: true,
    killfeed: true,
    callout: true,
};

pub(crate) fn colour(c: Rgba) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], c[3])
}

/// The window's size as the UI lays out (logical px).
fn screen_size(window: Query<&Window, With<PrimaryWindow>>, mut screen: ResMut<HudScreen>) {
    if let Ok(w) = window.single() {
        let s = Screen::new(w.resolution.width(), w.resolution.height());
        if s.w > 0.0 && s.h > 0.0 && screen.0 != s {
            screen.0 = s;
        }
    }
}

#[derive(Component)]
struct CrosshairRoot;
#[derive(Component)]
struct DamageRoot;
#[derive(Component)]
struct InfectedRoot;
#[derive(Component)]
struct StatusRoot;
#[derive(Component)]
struct WeaponRoot;
#[derive(Component)]
struct FeedRoot;
#[derive(Component)]
struct TagRoot;
#[derive(Component)]
struct InstallRoot;
#[derive(Component)]
struct MinigameRoot;
#[derive(Component)]
struct ActionRoot;
#[derive(Component)]
struct NoticeRoot;

/// The rest of the game's HUD panels bound by name (`bound`), when the
/// install's layouts have them: the kill feed, the score panel, the
/// pickup prompt and action gauge; and the HUD layout attributes'
/// textures the code draws (the damage overlay and direction indicator,
/// the kill feed's icons and badges, the action icons).
#[derive(Resource, Clone, Debug, Default)]
pub struct BoundHud {
    pub feed: Option<bound::FeedPanel>,
    pub score: Option<bound::ScorePanel>,
    pub action: Option<bound::ActionPanel>,
    /// `[HUDLayout0]` texture keys by attribute key (lower case).
    pub textures: HashMap<String, String>,
    /// The weapons' kill icons by id (`install::kill_icons`).
    pub kill_icons: std::sync::Arc<HashMap<String, String>>,
}

impl BoundHud {
    /// The bound panels of `ui`'s install data (none without it).
    pub fn of(ui: &CaUi) -> Self {
        let Some(inst) = ui.install.as_ref() else { return Self::default() };
        let hud = inst.hud.as_ref();
        Self {
            feed: hud.and_then(bound::FeedPanel::new),
            score: inst.indicator.as_ref().and_then(bound::ScorePanel::new),
            action: hud.and_then(bound::ActionPanel::new),
            textures: crate::ui::install::HUD_LAYOUT_ART
                .iter()
                .filter_map(|k| Some((k.to_string(), inst.hud_texture(k)?)))
                .collect(),
            kill_icons: inst.kill_icons.clone(),
        }
    }

    /// A `[HUDLayout0]` texture by key.
    pub fn texture(&self, key: &str) -> Option<&str> {
        self.textures.get(key).map(String::as_str)
    }

    /// A kill line's weapon slot texture (D5's replacements).
    pub fn icon(&self, icon: &KillIcon) -> Option<String> {
        let key = match icon {
            KillIcon::Weapon(Some(id)) if crate::ui::install::is_own_gun(id) => {
                return Some(crate::ui::install::kill_art(&self.kill_icons, id));
            }
            KillIcon::Weapon(Some(_)) => return None,
            KillIcon::Weapon(None) => return None,
            KillIcon::Suicide => "killsuicideimg",
            KillIcon::Barrel => "killdrumdeathimg",
            KillIcon::Fire => "killburndeathimg",
            KillIcon::Poison => "killpoisondeathimg",
            KillIcon::Bleeding => "killbleedingdeathimg",
            KillIcon::Npc => "killerisnpcimg",
        };
        self.texture(key).map(str::to_string)
    }

    /// A badge's texture (D5's order).
    pub fn badge(&self, b: Badge) -> Option<String> {
        let key = match b {
            Badge::Revenge => "killrevengeimg",
            Badge::Headshot => "killheadshotimg",
            Badge::Groin => "killnutshotimg",
            Badge::Flag => "killenemyflagimg",
            Badge::TeamFlag => "killteamflagimg",
            Badge::Gold => "killenemygoldimg",
            Badge::IceCrush => "killicecrushimg",
            Badge::Vip | Badge::VipRank => "killlogviprankimg1",
        };
        self.texture(key).map(str::to_string).or_else(|| badge_art(b))
    }
}

/// The game's HUD panels bound by name (`install`), when the install's
/// HUD layout reads: they draw the health and weapon panels in place of
/// ours.
#[derive(Resource, Clone, Debug, Default)]
pub struct InstallHud(pub Option<install::HudPanels>);

pub(crate) fn layer(z: i32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        GlobalZIndex(z),
    )
}

/// The HUD's top layer: under the flash's white (grenades.md section 10:
/// the HUD is drawn first, the white over it; chat and menus over both).
pub const HUD_Z: i32 = 28;

fn spawn(mut commands: Commands) {
    commands.spawn((ActionRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((CrosshairRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((DamageRoot, layer(HUD_Z - 1), HudRoot(crate::game::GAME)));
    commands.spawn((InfectedRoot, layer(HUD_Z - 3), HudRoot(crate::game::GAME)));
    commands.spawn((StatusRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((WeaponRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((FeedRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((TagRoot, layer(HUD_Z - 2), HudRoot(crate::game::GAME)));
    commands.spawn((InstallRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((MinigameRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
    commands.spawn((NoticeRoot, layer(HUD_Z), HudRoot(crate::game::GAME)));
}

/// The game's panels (`InstallHud`) with this frame's values.
fn draw_install_panels(
    hud: Res<CaHud>,
    params: PenParams,
    art: Option<Res<draw::CaArt>>,
    mut panels: ResMut<InstallHud>,
    root: Single<Entity, With<InstallRoot>>,
    mut last: Local<Option<(Option<StatusView>, Option<WeaponPanel>, Screen, bool)>>,
    mut commands: Commands,
) {
    let Some(p) = panels.0.as_mut() else { return };
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.status.clone(), hud.weapon.clone(), hud.screen, loaded), *root, &mut commands) {
        return;
    }
    p.bind(hud.status.as_ref(), hud.weapon.as_ref(), scale(hud.screen));
    let ui = &params.0;
    let draws = p.layout.draw(&ui.ctx());
    let (fonts, faces) = (params.1.as_deref(), params.2.as_deref());
    commands.entity(*root).with_children(|c| {
        draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), fonts, faces);
    });
}

/// The pickup prompt and the action gauge in the game's `Action` panel
/// (`BoundHud::action`), when the install has it.
#[allow(clippy::too_many_arguments)]
fn draw_action(
    hud: Res<CaHud>,
    score: Res<score::CaScoreHud>,
    params: PenParams,
    art: Option<Res<draw::CaArt>>,
    bound: Res<BoundHud>,
    root: Single<Entity, With<ActionRoot>>,
    mut last: Local<Option<(Option<ActionView>, Option<score::GaugeView>, Screen, bool)>>,
    mut commands: Commands,
) {
    let Some(panel) = &bound.action else { return };
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.action.clone(), score.gauge.clone(), hud.screen, loaded), *root, &mut commands) {
        return;
    }
    let gauge = score.gauge.as_ref().map(|g| {
        let icon = g.kind.icon_key().and_then(|k| bound.texture(k));
        (g.title.as_str(), g.percent, icon)
    });
    let ui = &params.0;
    let prompt = hud.action.as_ref().map(|a| a.text.as_str());
    let Some(layout) = panel.build(prompt, gauge, scale(hud.screen), &ui.fonts) else { return };
    let draws = layout.draw(&ui.ctx());
    commands.entity(*root).with_children(|c| {
        draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), params.1.as_deref(), params.2.as_deref());
    });
}

/// A rect node, px.
pub(crate) fn rect(r: PxRect, c: Color) -> impl Bundle {
    let s = r.size();
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(r.min.x),
            top: px(r.min.y),
            width: px(s.x),
            height: px(s.y),
            ..default()
        },
        BackgroundColor(c),
    )
}

/// A straight line node from `a` to `b`, `thickness` px.
fn line(a: Vec2, b: Vec2, thickness: f32, c: Color) -> impl Bundle {
    let mid = (a + b) / 2.0;
    let d = b - a;
    let len = d.length();
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(mid.x - len / 2.0),
            top: px(mid.y - thickness / 2.0),
            width: px(len),
            height: px(thickness),
            ..default()
        },
        UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
        BackgroundColor(c),
    )
}

/// What HUD texts are drawn with: the UI's fonts, the shell's stand-in
/// faces and the install's, on the HUD's screen.
#[derive(Clone, Copy)]
pub(crate) struct Pen<'a> {
    ui: &'a CaUi,
    fonts: Option<&'a UiFonts>,
    faces: Option<&'a CaFaces>,
    screen: Screen,
}

/// A HUD text (ui_system.md T2-T6, settling hud_crosshair.md Q2 and Q3):
/// `string` in `font` scaled with both factors, in a box (`at`, `size`,
/// ref px) at `own` inside it, aligned in the box on each aligned axis
/// (its own position ignored there), drawn with its effect's copies.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hud_text(
    p: &mut ChildSpawnerCommands,
    pen: Pen,
    (at, size, own): (Vec2, Vec2, Vec2),
    string: &str,
    font: &str,
    (h, v): (HAlign, VAlign),
    effect: Effect,
    colour: Rgba,
) {
    let look = TextLook {
        font: font.to_string(),
        h,
        v,
        effect,
        colour,
        stroke: [0, 0, 0, 255],
        shadow: [0, 0, 0, 255],
        set: Some(string.to_string()),
        ..default()
    };
    if let Some(t) = layout::boxed_text(
        at.as_ivec2(),
        size.as_ivec2(),
        own.as_ivec2(),
        look,
        scale(pen.screen),
        &pen.ui.fonts,
    ) {
        draw::spawn_text(p, &t, pen.fonts, pen.faces);
    }
}

/// A text at a reference position (default alignment).
fn hud_text_at(p: &mut ChildSpawnerCommands, pen: Pen, at: Vec2, string: &str, font: &str, effect: Effect, colour: Rgba) {
    hud_text(
        p,
        pen,
        (Vec2::ZERO, Vec2::ZERO, at),
        string,
        font,
        (HAlign::Default, VAlign::Default),
        effect,
        colour,
    );
}

/// Redraw a layer's children when what it shows changed.
pub(crate) fn rebuild<T: PartialEq + Clone + Send + Sync + 'static>(
    last: &mut Option<T>,
    now: T,
    root: Entity,
    commands: &mut Commands,
) -> bool {
    if last.as_ref() == Some(&now) {
        return false;
    }
    *last = Some(now);
    commands.entity(root).despawn_related::<Children>();
    true
}

/// The range readout's text (A9, Corrections 15): `RANGE_FONT` (the font
/// list's `SysFont13` at 20 px) scaled with the screen, its right end and
/// top at (490, 400) ref.
pub fn range_text(fonts: &FontList, screen: Screen, text: &str, colour: Rgba) -> Option<layout::TextDraw> {
    let look = TextLook {
        font: RANGE_FONT.to_string(),
        h: HAlign::Right,
        v: VAlign::Default,
        effect: Effect::Shadow,
        colour,
        shadow: [0, 0, 0, 255],
        set: Some(text.to_string()),
        ..default()
    };
    let width = 200;
    let at = RANGE_AT.as_ivec2() - IVec2::new(width, 0);
    layout::boxed_text(at, IVec2::new(width, 0), IVec2::ZERO, look, scale(screen), fonts)
}

/// The weapon's reticle image over the A8 rect (an outline while the
/// install has no such image).
fn spawn_reticle(p: &mut ChildSpawnerCommands, c: &CrosshairView, art: Option<&draw::CaArt>) {
    let Some(r) = c.dot_sight else { return };
    if let Some((image, size)) = c.dot_sight_image.as_deref().and_then(|n| art?.get(n)) {
        let full = Rect::new(0.0, 0.0, size.x as f32, size.y as f32);
        draw::image_part(p, image, [r.min.x, r.min.y, r.max.x, r.max.y], full, [255; 4]);
    } else {
        // PLACEHOLDER: a red outline.
        p.spawn((rect(r, Color::NONE), Outline::new(px(1.0), px(0.0), Color::srgba(1.0, 0.1, 0.1, 0.5))));
    }
}

fn draw_crosshair(
    hud: Res<CaHud>,
    params: PenParams,
    art: Option<Res<draw::CaArt>>,
    root: Single<Entity, With<CrosshairRoot>>,
    mut last: Local<Option<(CrosshairView, Screen, bool)>>,
    mut commands: Commands,
) {
    let c = &hud.crosshair;
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (c.clone(), hud.screen, loaded), *root, &mut commands) {
        return;
    }
    let col = colour(c.colour);
    let pen = pen(&params, hud.screen);
    commands.entity(*root).with_children(|p| {
        // The range readout: its right end and top at `RANGE_AT` (A9).
        if let Some((t, rgba)) = &c.range
            && let Some(d) = range_text(&pen.ui.fonts, hud.screen, t, *rgba)
        {
            draw::spawn_text(p, &d, pen.fonts, pen.faces);
        }
        // The gauge sight's images (the install's) beside the centre,
        // health left, clip right (ours: where; the special-weapon HUD
        // layout's `OptionCrossHair` isn't read).
        let images = c
            .gauge_images
            .as_ref()
            .and_then(|(l, r)| Some((art.as_deref()?.get(l)?.clone(), art.as_deref()?.get(r)?.clone())));
        if let Some(((left, ls), (right, rs))) = images {
            for ((image, size), right_side) in [((left, ls), false), ((right, rs), true)] {
                let size = size.as_vec2();
                let x = if right_side { 512.0 + 90.0 } else { 512.0 - 90.0 - size.x };
                let r = hud.screen.rect(Vec2::new(x, 384.0 - size.y / 2.0), size);
                let full = Rect::new(0.0, 0.0, size.x, size.y);
                draw::image_part(p, &image, [r.min.x, r.min.y, r.max.x, r.max.y], full, [255; 4]);
            }
        } else if let Some((hp, clip)) = c.gauge {
            // PLACEHOLDER for the gauge sight's images: their numbers.
            let size = Vec2::new(60.0, 16.0);
            let white = [255, 255, 255, 220];
            let left = (Vec2::new(512.0 - 90.0 - size.x, 376.0), size, Vec2::ZERO);
            let right = (Vec2::new(512.0 + 90.0, 376.0), size, Vec2::ZERO);
            let (l, r) = ((HAlign::Right, VAlign::Centre), (HAlign::Left, VAlign::Centre));
            hud_text(p, pen, left, &format!("HP {hp}"), looks::ICON_WORD, l, Effect::Shadow, white);
            hud_text(p, pen, right, &format!("{clip} AMMO"), looks::ICON_WORD, r, Effect::Shadow, white);
        }
        for _ in 0..c.draws.dot_sight {
            spawn_reticle(p, c, art.as_deref());
        }
        for e in &c.elements {
            match e {
                Element::Rect(r) => {
                    p.spawn(rect(*r, col));
                }
                Element::Line(a, b) => {
                    p.spawn(line(*a, *b, 1.0, col));
                }
                Element::Circle { centre, radius } => {
                    let v = circle_vertices(*centre, *radius);
                    for k in 0..v.len() {
                        p.spawn(line(v[k], v[(k + 1) % v.len()], 1.0, col));
                    }
                }
            }
        }
    });
}

/// The infected's screen texture over its rect, white, alpha-blended
/// (game_modes.md Corrections (2026-10-30) 3); nothing without the
/// install's texture.
fn draw_infected(
    hud: Res<CaHud>,
    art: Option<Res<draw::CaArt>>,
    bound: Res<BoundHud>,
    root: Single<Entity, With<InfectedRoot>>,
    mut last: Local<Option<(Option<PxRect>, bool)>>,
    mut commands: Commands,
) {
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.infected, loaded), *root, &mut commands) {
        return;
    }
    let Some(r) = hud.infected else { return };
    let Some((image, size)) = art.as_deref().and_then(|art| art.get(bound.texture("infectedtex")?).cloned()) else { return };
    commands.entity(*root).with_children(|p| {
        let full = Rect::new(0.0, 0.0, size.x as f32, size.y as f32);
        draw::image_part(p, &image, [r.min.x, r.min.y, r.max.x, r.max.y], full, [255; 4]);
    });
}

fn draw_damage(
    hud: Res<CaHud>,
    art: Option<Res<draw::CaArt>>,
    bound: Res<BoundHud>,
    root: Single<Entity, With<DamageRoot>>,
    mut last: Local<Option<(Option<DamageView>, bool)>>,
    mut commands: Commands,
) {
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.damage.clone(), loaded), *root, &mut commands) {
        return;
    }
    let Some(d) = &hud.damage else { return };
    let a = d.alpha as f32 / 255.0;
    // The HUD layout attributes' images (`damagetex`, the hit overlay;
    // `damageindicatortex`, the direction indicator), when the install
    // has them.
    let image = |key: &str| art.as_deref().and_then(|art| art.get(bound.texture(key)?).cloned());
    if let (Some((hit, hit_size)), Some((arrow, arrow_size))) = (image("damagetex"), image("damageindicatortex")) {
        let tint = [255, 255, 255, d.alpha];
        commands.entity(*root).with_children(|p| {
            let o = d.overlay;
            let rect = Rect::new(0.0, 0.0, hit_size.x as f32, hit_size.y as f32);
            draw::image_part(p, &hit, [o.min.x, o.min.y, o.max.x, o.max.y], rect, tint);
            spawn_indicator(p, d.corners, &arrow, arrow_size, tint);
        });
        return;
    }
    commands.entity(*root).with_children(|p| {
        // PLACEHOLDER for the red edge vignette: a red frame over the
        // overlay's rect (1024 ref tall: its bottom is off the screen).
        let edge = 0.06 * hud.screen.h;
        let s = d.overlay.size();
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(d.overlay.min.x),
                top: px(d.overlay.min.y),
                width: px(s.x),
                height: px(s.y),
                border: UiRect::all(px(edge)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.8, 0.0, 0.0, 0.45 * a)),
        ));
        // PLACEHOLDER for the arc: a bar along the square's top edge
        // (P0 to P1, the side facing the source), a little inside.
        let [p0, p1, ..] = d.corners;
        let centre = hud.screen.at(Vec2::new(512.0, 384.0));
        let (a0, a1) = (p0.lerp(centre, 0.12), p1.lerp(centre, 0.12));
        let (q0, q1) = (a0.lerp(a1, 0.25), a0.lerp(a1, 0.75));
        p.spawn(line(q0, q1, 6.0 * hud.screen.yr(), Color::srgba(0.85, 0.05, 0.05, 0.9 * a)));
    });
}

/// The direction indicator's image on its square (C): P0, P1 and P3
/// carry the texture's top-left, top-right and bottom-left corners (a
/// mirrored image when they turn the other way), its texels u in [0,
/// 338/512], v in [0, 338/256]: the rows past the texture's last left
/// empty (no clamp addressing); a square stretched by x_r and y_r is
/// drawn as the rectangle of its sides' lengths (no shear).
fn spawn_indicator(p: &mut ChildSpawnerCommands, corners: [Vec2; 4], image: &Handle<Image>, size: IVec2, tint: Rgba) {
    let [p0, p1, _, p3] = corners;
    let (u, v) = (p1 - p0, p3 - p0);
    let (w, h) = (u.length(), v.length());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let mirrored = u.x * v.y - u.y * v.x < 0.0;
    // The node's left-to-right axis: the image's when not mirrored, else
    // reversed (its top-left drawn at the node's top-right).
    let axis = if mirrored { -u } else { u };
    let centre = (p0 + p1 + corners[2] + p3) / 4.0;
    let (uw, vh) = (UV_SIDE / size.x.max(1) as f32, UV_SIDE / size.y.max(1) as f32);
    // The share of the square's height the texture's rows cover.
    let rows = (1.0 / vh).min(1.0);
    let rect = Rect::new(0.0, 0.0, (uw * size.x as f32).min(size.x as f32), size.y as f32);
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(centre.x - w / 2.0),
            top: px(centre.y - h / 2.0),
            width: px(w),
            height: px(h),
            ..default()
        },
        UiTransform::from_rotation(Rot2::radians(axis.y.atan2(axis.x))),
    ))
    .with_children(|c| {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0 * rows),
                ..default()
            },
            ImageNode {
                image: image.clone(),
                color: colour(tint),
                rect: Some(rect),
                flip_x: mirrored,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
        ));
    });
}

/// The direction indicator's square side in texels (C: S = 338).
const UV_SIDE: f32 = 338.0;

/// The HUD texts' fonts and looks (hud_crosshair.md D2, D4, D5).
mod looks {
    pub const LABEL: &str = "bold 16";
    pub const VALUE: &str = "number 25";
    pub const NAME: &str = "bold 16";
    pub const CLIP: &str = "number 26";
    pub const SLASH: &str = "bold 25";
    pub const RESERVE: &str = "number 25";
    /// Ours: placeholder words for icons, and the pickup prompt.
    pub const ICON_WORD: &str = "bold 11";
    pub const PROMPT: &str = "bold 14";
    /// Ours: a minigame's panel (name, status lines, notices).
    pub const MINIGAME_TITLE: &str = "bold 16";
    pub const MINIGAME: &str = "bold 14";
    pub const NOTICE: &str = "bold 20";
}

pub(crate) type PenParams<'a> = (Res<'a, CaUi>, Option<Res<'a, UiFonts>>, Option<Res<'a, CaFaces>>);

pub(crate) fn pen<'a>(params: &'a PenParams, screen: Screen) -> Pen<'a> {
    Pen {
        ui: &params.0,
        fonts: params.1.as_deref(),
        faces: params.2.as_deref(),
        screen,
    }
}

fn draw_status(
    hud: Res<CaHud>,
    params: PenParams,
    panels: Res<InstallHud>,
    bound: Res<BoundHud>,
    root: Single<Entity, With<StatusRoot>>,
    mut last: Local<Option<(Option<StatusView>, Option<ActionView>, Screen)>>,
    mut commands: Commands,
) {
    if !rebuild(&mut last, (hud.status.clone(), hud.action.clone(), hud.screen), *root, &mut commands) {
        return;
    }
    let screen = hud.screen;
    let pen = pen(&params, screen);
    let resolve = |id: &str| pen.ui.strings.resolve(id);
    commands.entity(*root).with_children(|p| {
        if let Some(prompt) = hud.action.as_ref().map(|a| &a.text).filter(|_| bound.action.is_none()) {
            // Ours without the install's `Action` panel (D9 places it
            // there).
            hud_text(
                p,
                pen,
                (Vec2::new(312.0, 430.0), Vec2::new(400.0, 20.0), Vec2::ZERO),
                prompt,
                looks::PROMPT,
                (HAlign::Centre, VAlign::Centre),
                Effect::Shadow,
                [255, 255, 255, 255],
            );
        }
        let Some(s) = &hud.status else { return };
        if panels.0.is_some() {
            return;
        }
        let box_of = |(at, size): (Vec2, Vec2)| screen.rect(at, size);
        // PLACEHOLDER backgrounds for the panel's textures.
        p.spawn(rect(box_of(status::PANEL), Color::srgba(0.05, 0.06, 0.07, 0.65)));
        for plate in status::PLATES {
            p.spawn(rect(box_of(plate), Color::srgba(0.0, 0.0, 0.0, 0.45)));
        }
        // PLACEHOLDER silhouette: a box, shorter when crouched, tinted red
        // at low health.
        let sil = box_of(status::SILHOUETTE);
        let tall = if s.crouched { 0.3 } else { 0.48 };
        let w = sil.size().x * 0.3;
        let h = sil.size().y * tall;
        let bottom = screen.at(Vec2::new(0.0, 766.0)).y;
        let x = sil.min.x + (sil.size().x - w) / 2.0;
        let tint = s.colours.silhouette.map_or(Color::srgba(0.75, 0.75, 0.75, 0.8), colour);
        p.spawn(rect(PxRect::new(x, x + w, bottom - h, bottom), tint));
        // Labels at their positions; values centred in their boxes.
        for (at, id, c) in [
            (status::HP_LABEL, "$hud_hp", s.colours.hp_ap_labels),
            (status::AP_LABEL, "$hud_ap", s.colours.hp_ap_labels),
            (status::SP_LABEL, "$hud_sp", s.colours.other_labels),
            (status::SET_LABEL, "$hud_set", s.colours.other_labels),
        ] {
            hud_text_at(p, pen, at, &resolve(id), looks::LABEL, Effect::Shadow, c);
        }
        for ((at, size), t) in [(status::HP_BOX, &s.hp), (status::AP_BOX, &s.ap)] {
            hud_text(
                p,
                pen,
                (at, size, Vec2::ZERO),
                t,
                looks::VALUE,
                (HAlign::Centre, VAlign::Centre),
                Effect::Shadow,
                s.colours.values,
            );
        }
        if s.stamina_bar > 0.0 {
            p.spawn(rect(
                screen.rect(status::STAMINA_BAR, Vec2::new(s.stamina_bar, status::STAMINA_BAR_HEIGHT)),
                Color::srgba(0.45, 0.75, 0.95, 0.85),
            ));
        }
    });
}

fn draw_weapon(
    hud: Res<CaHud>,
    params: PenParams,
    panels: Res<InstallHud>,
    root: Single<Entity, With<WeaponRoot>>,
    mut last: Local<Option<(Option<WeaponPanel>, Screen)>>,
    mut commands: Commands,
) {
    if !rebuild(&mut last, (hud.weapon.clone(), hud.screen), *root, &mut commands) {
        return;
    }
    let Some(w) = &hud.weapon else { return };
    if panels.0.is_some() {
        return;
    }
    let pen = pen(&params, hud.screen);
    commands.entity(*root).with_children(|p| {
        // Right-aligned in its box, centred on its y (Q3).
        let (at, size) = panel::NAME_BOX;
        hud_text(
            p,
            pen,
            (at, size, Vec2::ZERO),
            &w.name,
            looks::NAME,
            (HAlign::Right, VAlign::Centre),
            Effect::Stroke,
            panel::NAME,
        );
        if let Some((clip, reserve)) = &w.ammo {
            hud_text_at(p, pen, panel::CLIP_AT, clip, looks::CLIP, Effect::Stroke, panel::CLIP);
            hud_text_at(p, pen, panel::SLASH_AT, "/", looks::SLASH, Effect::Stroke, panel::RESERVE);
            hud_text_at(p, pen, panel::RESERVE_AT, reserve, looks::RESERVE, Effect::Stroke, panel::RESERVE);
        }
        if let Some(icon) = w.icon {
            // PLACEHOLDER for the fire-mode icon: its name.
            let t = match icon {
                FireIcon::Burst2 => "BURST 2",
                FireIcon::Burst3 => "BURST 3",
                FireIcon::Auto => "AUTO",
                FireIcon::Single => "SEMI",
            };
            hud_text_at(p, pen, panel::ICON.0, t, looks::ICON_WORD, Effect::None, panel::RESERVE);
        }
    });
}

fn draw_feed(
    hud: Res<CaHud>,
    params: PenParams,
    art: Option<Res<draw::CaArt>>,
    bound: Res<BoundHud>,
    (icons, names, catalog): (Res<WeaponIcons>, Option<Res<GunNames>>, Option<Res<crate::weapon::data::WeaponCatalog>>),
    root: Single<Entity, With<FeedRoot>>,
    mut last: Local<Option<(Vec<FeedLineView>, Screen, bool, bool)>>,
    mut commands: Commands,
) {
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.feed.clone(), hud.screen, loaded, icons.is_changed()), *root, &mut commands) {
        return;
    }
    let foreign = |p: &mut ChildSpawnerCommands, pen: Pen, x: f32, y: f32, id: &str| {
        foreign_weapon(p, pen, &hud.screen, Vec2::new(x, y), id, &icons, names.as_deref(), catalog.as_deref());
    };
    // The game's own kill feed panel, bound by name (`bound`).
    if let Some(panel) = &bound.feed {
        let feed_art = bound::FeedArt {
            icon: &|i| bound.icon(i),
            badge: &|b| bound.badge(b),
        };
        let layout = panel.build(&hud.feed, &feed_art, scale(hud.screen));
        let ui = &params.0;
        let draws = layout.draw(&ui.ctx());
        let pen = pen(&params, hud.screen);
        commands.entity(*root).with_children(|c| {
            draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), params.1.as_deref(), params.2.as_deref());
            for l in &hud.feed {
                for item in &l.items {
                    if let FeedItem::Icon { x, icon } = item
                        && let Some(id) = foreign_gun(icon)
                    {
                        foreign(c, pen, *x, l.y, id);
                    }
                }
            }
        });
        return;
    }
    let pen = pen(&params, hud.screen);
    let grey = [200, 200, 200, FEED_TEXT_ALPHA];
    commands.entity(*root).with_children(|p| {
        for l in &hud.feed {
            // Each element at the running x (`feed_line`, names advancing by
            // their measured width), 3 ref px down the line.
            for item in &l.items {
                match item {
                    FeedItem::Text { x, text, colour } => {
                        hud_text_at(p, pen, Vec2::new(*x, l.y + 3.0), text, FEED_FONT, Effect::Stroke, *colour);
                    }
                    // Another game's gun: its own game's icon, else its name.
                    FeedItem::Icon { x, icon } if foreign_gun(icon).is_some() => {
                        foreign(p, pen, *x, l.y, foreign_gun(icon).unwrap());
                    }
                    // The install's icons and badges when it has them
                    // (64 x 20 and 32 x 20 ref, D5).
                    FeedItem::Icon { x, icon } if feed_art(icon_art(icon, &bound.kill_icons), art.as_deref()).is_some() => {
                        let (image, size) = feed_art(icon_art(icon, &bound.kill_icons), art.as_deref()).unwrap();
                        let r = hud.screen.rect(Vec2::new(*x, l.y), Vec2::new(FEED_ICON_WIDTH, 20.0));
                        let rect = Rect::new(0.0, 0.0, (size.x as f32).min(64.0), (size.y as f32).min(20.0));
                        draw::image_part(p, &image, [r.min.x, r.min.y, r.max.x, r.max.y], rect, [255; 4]);
                    }
                    FeedItem::Badge { x, badge } if feed_art(badge_art(*badge), art.as_deref()).is_some() => {
                        let (image, size) = feed_art(badge_art(*badge), art.as_deref()).unwrap();
                        let r = hud.screen.rect(Vec2::new(*x, l.y), Vec2::new(FEED_BADGE_WIDTH, 20.0));
                        let rect = Rect::new(0.0, 0.0, (size.x as f32).min(32.0), (size.y as f32).min(20.0));
                        draw::image_part(p, &image, [r.min.x, r.min.y, r.max.x, r.max.y], rect, [255; 4]);
                    }
                    // PLACEHOLDER icons and badges: words in their slots.
                    FeedItem::Icon { x, icon } => {
                        let word = match icon {
                            KillIcon::Weapon(Some(id)) => format!("[{}]", id.rsplit(':').next().unwrap_or(id)),
                            KillIcon::Weapon(None) => String::new(),
                            KillIcon::Suicide => "[suicide]".into(),
                            KillIcon::Barrel => "[blast]".into(),
                            KillIcon::Fire => "[fire]".into(),
                            KillIcon::Poison => "[poison]".into(),
                            KillIcon::Bleeding => "[bleed]".into(),
                            KillIcon::Npc => "[npc]".into(),
                        };
                        hud_text_at(p, pen, Vec2::new(*x, l.y + 4.0), &word, looks::ICON_WORD, Effect::Stroke, grey);
                    }
                    FeedItem::Badge { x, badge } => {
                        let word = format!("{badge:?}").chars().take(2).collect::<String>().to_uppercase();
                        hud_text_at(p, pen, Vec2::new(*x, l.y + 4.0), &word, looks::ICON_WORD, Effect::Stroke, grey);
                    }
                }
            }
        }
    });
}

/// A kill line's weapon slot's texture in the install (`ui::install::
/// kill_art`, by the weapons' kill icons `kills`); none for another
/// game's gun (`foreign_gun`).
fn icon_art(icon: &KillIcon, kills: &HashMap<String, String>) -> Option<String> {
    match icon {
        KillIcon::Weapon(Some(id)) if foreign_gun(icon).is_none() => Some(crate::ui::install::kill_art(kills, id)),
        _ => None,
    }
}

/// The id of a kill line's gun when it is another game's.
fn foreign_gun(icon: &KillIcon) -> Option<&str> {
    match icon {
        KillIcon::Weapon(Some(id)) if !crate::ui::install::is_own_gun(id) => Some(id),
        _ => None,
    }
}

/// Draws another game's gun in a kill line's weapon slot (at `at`, in
/// reference pixels): that game's icon (`WeaponIcons`), else the gun's
/// name, cut to the slot.
#[allow(clippy::too_many_arguments)]
fn foreign_weapon(
    p: &mut ChildSpawnerCommands,
    pen: Pen,
    screen: &Screen,
    at: Vec2,
    id: &str,
    icons: &WeaponIcons,
    names: Option<&GunNames>,
    catalog: Option<&crate::weapon::data::WeaponCatalog>,
) {
    let r = screen.rect(at, Vec2::new(FEED_ICON_WIDTH, 20.0));
    match icons.icon(id) {
        Some(icon) => {
            let tall = r.max.y - r.min.y;
            spawn_weapon_icon(p, &icon, tall / 20.0, tall, Some(Vec2::new(r.min.x, r.min.y)));
        }
        None => {
            let name: String = gun_name(id, names, catalog).chars().take(10).collect();
            hud_text_at(p, pen, Vec2::new(at.x, at.y + 3.0), &name, FEED_FONT, Effect::Stroke, [200, 200, 200, FEED_TEXT_ALPHA]);
        }
    }
}

/// Registers the install's kill icons for other games' HUDs
/// (`WeaponIcons`), as the art loads: each weapon's own (`install::
/// kill_art`: the install's copies by their record's icon).
fn register_icons(art: Option<Res<draw::CaArt>>, bound: Option<Res<BoundHud>>, mut icons: ResMut<WeaponIcons>) {
    let Some(art) = art.filter(|a| a.is_changed() && !a.0.is_empty()) else { return };
    let by_id = bound.map(|b| b.kill_icons.clone()).unwrap_or_default();
    if let Some((image, size)) = badge_art(Badge::Headshot).and_then(|k| art.get(&k)) {
        icons.register_mark(
            crate::game::GAME,
            crate::client::kill_feed::HEADSHOT,
            WeaponIcon::Image { image: image.clone(), size: size.as_vec2() },
        );
    }
    icons.register(crate::game::GAME, icon_resolver(&art, by_id));
}

/// Our weapon ids' kill icons from `art` (`WeaponIcons`' resolver): each
/// weapon's own (`install::kill_art` by the weapons' kill icons
/// `by_id`), none for a gun without art.
pub fn icon_resolver(
    art: &draw::CaArt,
    by_id: std::sync::Arc<HashMap<String, String>>,
) -> impl Fn(&str) -> Option<WeaponIcon> + Send + Sync + 'static {
    let kills: HashMap<String, (Handle<Image>, IVec2)> = art
        .0
        .iter()
        .filter(|(k, _)| k.contains("/killdeath/"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    move |id| {
        let (image, size) = kills.get(&crate::ui::install::kill_art(&by_id, id))?;
        Some(WeaponIcon::Image { image: image.clone(), size: size.as_vec2() })
    }
}

/// A badge's texture in the install, where it has one.
fn badge_art(b: Badge) -> Option<String> {
    let f = match b {
        Badge::Revenge => "revenge",
        Badge::Headshot => "headshot",
        Badge::Groin => "nutshot",
        Badge::TeamFlag => "flag_log_team",
        Badge::Vip => "vip_vip",
        _ => return None,
    };
    Some(format!("ui_hud_message/killdeath/icon/{f}.dtx"))
}

fn feed_art(name: Option<String>, art: Option<&draw::CaArt>) -> Option<(Handle<Image>, IVec2)> {
    art?.get(&name?).cloned()
}

fn draw_tags(
    hud: Res<CaHud>,
    fonts: Option<Res<UiFonts>>,
    root: Single<Entity, With<TagRoot>>,
    camera: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
    mut commands: Commands,
) {
    commands.entity(*root).despawn_related::<Children>();
    let Some((cam, at)) = camera.iter().next() else { return };
    let pts = 12.0 * hud.screen.yr();
    commands.entity(*root).with_children(|p| {
        for t in &hud.tags {
            // No distance limit: only a tag behind the camera hides.
            let depth = (t.anchor - at.translation()).dot(at.forward().as_vec3());
            if !tag_in_front(depth) {
                continue;
            }
            let Ok(s) = cam.world_to_viewport(at, t.anchor) else {
                continue;
            };
            let opacity = t.tag.opacity as f32 / 255.0;
            let [r, g, b, a] = t.tag.name;
            let name = Color::srgba_u8(r, g, b, (a as f32 * opacity) as u8);
            let font = fonts.as_deref().map_or_else(
                || TextFont {
                    font_size: FontSize::Px(pts),
                    ..default()
                },
                |f| f.em("Tahoma", true, pts),
            );
            let (w, h) = (200.0, pts * 2.4);
            let mut tag = p.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(s.x - w / 2.0),
                top: px(s.y - h),
                width: px(w),
                height: px(h),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::End,
                ..default()
            });
            tag.with_children(|c| {
                // PLACEHOLDER marks for the tag's icons (rank left of the
                // name, flag and revenge after it).
                let mut line = t.name.clone();
                if t.tag.rank {
                    line = format!("[*] {line}");
                }
                if t.tag.flag {
                    line.push_str(" [F]");
                }
                if t.tag.revenge {
                    line.push_str(" [R]");
                }
                c.spawn((Text::new(line), font, TextColor(name), TextLayout::no_wrap()));
                if t.tag.hp_bar {
                    c.spawn((
                        Node {
                            width: px(40.0),
                            height: px(3.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6 * opacity)),
                        children![(
                            Node {
                                width: percent(100.0 * t.health.clamp(0.0, 1.0)),
                                height: percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.3, 0.9, 0.3, opacity)),
                        )],
                    ));
                }
            });
        }
    });
}

/// The minigame's panel: its name and status lines at the top centre, its
/// callout (a rung up, the lead, the match won: the shell's is hidden
/// while this HUD is up, `DRAWS`) under the middle of the view.
/// Ours (the install's HUD has no such panel), drawn in the HUD's own text
/// looks.
fn draw_minigame(
    hud: Res<CaHud>,
    params: PenParams,
    root: Single<Entity, With<MinigameRoot>>,
    mut last: Local<Option<(Option<MinigameView>, Screen)>>,
    mut commands: Commands,
) {
    if !rebuild(&mut last, (hud.minigame.clone(), hud.screen), *root, &mut commands) {
        return;
    }
    let Some(m) = &hud.minigame else { return };
    let pen = pen(&params, hud.screen);
    commands.entity(*root).with_children(|p| {
        let centred = |p: &mut ChildSpawnerCommands, y: f32, h: f32, text: &str, font: &str, colour: Rgba| {
            hud_text(
                p,
                pen,
                (Vec2::new(262.0, y), Vec2::new(500.0, h), Vec2::ZERO),
                text,
                font,
                (HAlign::Centre, VAlign::Centre),
                Effect::Stroke,
                colour,
            );
        };
        centred(p, 8.0, 20.0, &m.title, looks::MINIGAME_TITLE, [255, 200, 60, 255]);
        for (i, line) in m.lines.iter().enumerate() {
            centred(p, 30.0 + 18.0 * i as f32, 18.0, line, looks::MINIGAME, [255, 255, 255, 255]);
        }
        // The results take the callout's place (they say who won).
        if m.results.is_empty() {
            for (i, line) in m.callout.iter().enumerate() {
                // Under the announcer's kill banner (the shell's, 76..332
                // on this canvas; its art ends near 250), above the
                // crosshair.
                if i == 0 {
                    centred(p, 284.0, 28.0, line, looks::NOTICE, [255, 230, 120, 255]);
                } else {
                    centred(p, 316.0 + 18.0 * (i - 1) as f32, 18.0, line, looks::MINIGAME, [255, 255, 255, 255]);
                }
            }
        }
        for (i, line) in m.results.iter().enumerate() {
            if i == 0 {
                centred(p, 110.0, 28.0, line, looks::NOTICE, [255, 230, 120, 255]);
            } else {
                centred(p, 142.0 + 18.0 * (i - 1) as f32, 18.0, line, looks::MINIGAME, [255, 255, 255, 255]);
            }
        }
    });
}

#[cfg(test)]
mod foreign_gun_tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;

    /// What `foreign_weapon` spawns under a root for gun `id`, and the
    /// registry it asks.
    fn drawn(id: &str, icons: &WeaponIcons) -> (World, Entity) {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let ui = CaUi::default();
        let pen = Pen { ui: &ui, fonts: None, faces: None, screen: Screen::default() };
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.entity(root).with_children(|p| {
            foreign_weapon(p, pen, &pen.screen, Vec2::new(40.0, 10.0), id, icons, None, None);
        });
        queue.apply(&mut world);
        (world, root)
    }

    fn images(world: &mut World) -> Vec<Node> {
        world.query_filtered::<&Node, With<ImageNode>>().iter(world).cloned().collect()
    }

    #[test]
    fn another_games_gun_is_its_icon_else_its_name() {
        let mut icons = WeaponIcons::default();
        icons.register("cs_source", |id| {
            id.ends_with("m4a1").then(|| WeaponIcon::Image { image: Handle::default(), size: Vec2::new(64.0, 20.0) })
        });
        let (mut world, _) = drawn("cs_source:weapon_m4a1", &icons);
        let nodes = images(&mut world);
        assert_eq!(nodes.len(), 1, "the icon is one image");
        assert_eq!(nodes[0].position_type, PositionType::Absolute);
        assert_eq!(nodes[0].left, px(40.0));
        // A gun its game has no icon for: the name, no image.
        let (mut world, root) = drawn("cs_source:weapon_awp", &icons);
        assert!(images(&mut world).is_empty());
        assert!(world.get::<Children>(root).is_some_and(|c| !c.is_empty()), "the name's text is up");
        // A game that registered nothing: the name too.
        let (mut world, root) = drawn("cs_source:weapon_m4a1", &WeaponIcons::default());
        assert!(images(&mut world).is_empty());
        assert!(world.get::<Children>(root).is_some_and(|c| !c.is_empty()));
    }

    /// A8: the reticle is the record's image over the 129 x 129 rect;
    /// without the image, the placeholder outline (no image node).
    #[test]
    fn the_reticle_is_the_records_image() {
        let screen = Screen::default();
        let view = CrosshairView {
            dot_sight: Some(dot_sight_rect(screen)),
            dot_sight_image: Some("Test\\Reticle.dtx".into()),
            ..default()
        };
        let mut art = draw::CaArt::default();
        art.0.insert(crate::ui::install::texture_key("test/reticle.dtx"), (Handle::default(), IVec2::new(128, 128)));
        let spawn = |art: Option<&draw::CaArt>, view: &CrosshairView| {
            let mut world = World::new();
            let root = world.spawn_empty().id();
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, &world);
            commands.entity(root).with_children(|p| spawn_reticle(p, view, art));
            queue.apply(&mut world);
            images(&mut world)
        };
        let nodes = spawn(Some(&art), &view);
        assert_eq!(nodes.len(), 1);
        assert_eq!((nodes[0].left, nodes[0].top), (px(448.0), px(320.0)));
        assert!(spawn(Some(&art), &CrosshairView { dot_sight_image: Some("other.dtx".into()), ..view.clone() }).is_empty());
        assert!(spawn(None, &view).is_empty());
        assert!(spawn(Some(&art), &CrosshairView { dot_sight: None, ..view }).is_empty());
    }

    #[test]
    fn the_install_art_is_for_its_own_guns_only() {
        let k = |id: &str| KillIcon::Weapon(Some(id.into()));
        let none = HashMap::new();
        assert_eq!(icon_art(&k("combat_arms:m4a1"), &none), Some(crate::ui::install::texture_key(&crate::ui::install::kill_icon("combat_arms:m4a1"))));
        assert_eq!(icon_art(&k("cs_source:weapon_m4a1"), &none), None);
        // An install copy: its record's icon.
        let kills: HashMap<String, String> = [("combat_arms:m4a1_gold".to_string(), "ui_hud_message/killdeath/weapon/kill_message_m4a1.dtx".to_string())].into();
        assert_eq!(icon_art(&k("combat_arms:m4a1_gold"), &kills).as_deref(), Some("ui_hud_message/killdeath/weapon/kill_message_m4a1.dtx"));
        assert_eq!(foreign_gun(&k("cs_source:weapon_m4a1")), Some("cs_source:weapon_m4a1"));
        assert_eq!(foreign_gun(&k("combat_arms:m4a1")), None);
        assert_eq!(foreign_gun(&KillIcon::Suicide), None);
    }

    #[test]
    fn the_install_art_registers_for_other_huds() {
        let mut app = App::new();
        let mut art = draw::CaArt::default();
        let key = crate::ui::install::texture_key(&crate::ui::install::kill_icon("combat_arms:m4a1"));
        art.0.insert(key, (Handle::default(), IVec2::new(64, 20)));
        app.init_resource::<WeaponIcons>().insert_resource(art).add_systems(Update, register_icons);
        app.update();
        let icons = app.world().resource::<WeaponIcons>();
        assert!(matches!(icons.icon("combat_arms:m4a1"), Some(WeaponIcon::Image { .. })));
        assert_eq!(icons.icon("combat_arms:no_such_gun"), None);
        assert_eq!(icons.icon("cs_source:weapon_m4a1"), None, "not our game's id");
    }
}
