//! What CS 1.6's view models play (`chimera_sim::weapon::view_anim`'s
//! rules for this game, `rules`): its `v_*.mdl` sequences by name, as the
//! install's models have them (specs/goldsrc/mdl.md's example; weapons.md,
//! "Reload, draw and switch times"):
//!
//! - draw `draw` (the Glock's `draw` or `draw2`, the grenades' `deploy`),
//!   idle `idle` or `idle1`;
//! - a shot `shoot1`-`shoot3` (or `shoot`), the last round `shootlast` or
//!   `shoot_empty`; the Glock `shoot3` alone, `shoot`/`shoot2` (three
//!   flashes) for a burst, whose follow-ups don't restart it; the Elites
//!   `shoot_left*` and `shoot_right*` by the clip's parity (the left gun
//!   first from a full clip), `shoot_leftlast`/`shoot_rightlast` as each
//!   empties;
//! - a silencer gun (`[alt] kind = "silencer"`) plays the `_unsil`
//!   sequences while the silencer is off (mode 0), `add_silencer` and
//!   `detach_silencer` as it goes on and off;
//! - `reload` (the Glock's `reload` or `reload2`); the shotguns
//!   `start_reload`, `insert` per shell, `after_reload`;
//! - the knife `midslash1`/`midslash2` in turn, `stab` or `stab_miss`;
//!   the grenades `pullpin` and `throw`.
//!
//! The animation events (mdl.md, "Events"): 5001, 5011, 5021 and 5031 a
//! muzzle flash at attachment 0-3, of the kind its code names
//! (`weapon_models::flash_kind`, the kit's from `weapons::add_guns`); 5004
//! the sound its options name (a
//! wave under `sound/`, played for the holder alone, as the game's client
//! does).
//!
//! Known shortcuts:
//! - The idles' choice among `idle1`-`idle3` (the Glock's) and their
//!   timing are ours (the data's `[anim]`).

use chimera_sim::weapon::{
    WeaponEventKind,
    data::AltData,
    view_anim::{Pick, ViewAnimRules, ViewFrame, ViewState},
};

use crate::map::{GameFov, ViewModelEventKind, anim::AnimEvent};

/// CS 1.6's view models' field of view: the player's own (GoldSrc draws
/// them with the view's 90, zoomed with it).
pub fn fov() -> GameFov {
    GameFov { game: super::super::GAME, fov: 90.0, default_fov: 90.0, follows_player: true }
}

/// CS 1.6's view-model rules (module doc).
pub fn rules() -> ViewAnimRules {
    ViewAnimRules { draw, idle, event, effect, draw_rate: 1.0, others_reload: None }
}

/// The weapon's name after its game (`ak47`, `weapon_ak47` → `ak47`).
fn short(s: &ViewState) -> &'static str {
    let id = s.weapon.rsplit(':').next().unwrap_or(s.weapon);
    id.strip_prefix("weapon_").unwrap_or(id)
}

/// A silencer gun with its silencer off: the `_unsil` sequences.
fn unsilenced(s: &ViewState) -> bool {
    s.mode == 0 && s.data.and_then(|d| d.alt.as_ref()).is_some_and(|a| matches!(a, AltData::Silencer { .. }))
}

fn names(list: &[&str], s: &ViewState) -> Pick {
    let sfx = if unsilenced(s) { "_unsil" } else { "" };
    Pick::Names(list.iter().map(|n| format!("{n}{sfx}")).collect())
}

fn grenade(s: &ViewState) -> bool {
    matches!(short(s), "hegrenade" | "flashbang" | "smokegrenade")
}

fn draw(s: &ViewState) -> Pick {
    match short(s) {
        "glock18" => names(&["draw", "draw2"], s),
        _ if grenade(s) => names(&["deploy"], s),
        _ => names(&["draw"], s),
    }
}

fn idle(s: &ViewState) -> Pick {
    names(&["idle", "idle1"], s)
}

/// Seconds a burst's sequence is left to play its follow-ups.
const BURST_HOLD: f64 = 0.2;

fn event(frame: &mut ViewFrame, kind: &WeaponEventKind) {
    let s = &frame.state;
    let pick = match kind {
        WeaponEventKind::Fired { .. } if !frame.fired => {
            // A burst's later rounds don't restart its sequence.
            if s.burst.is_some() && frame.now - frame.dice.burst_started < BURST_HOLD {
                return;
            }
            let empty = s.clip == Some(0);
            match short(s) {
                "glock18" if empty => names(&["shoot_empty"], s),
                "glock18" if s.burst.is_some() => names(&["shoot", "shoot2"], s),
                "glock18" => names(&["shoot3"], s),
                "elite" => {
                    // The shot that leaves an odd count was the left gun's.
                    let clip = s.clip.unwrap_or(0);
                    let left = clip % 2 == 1;
                    let n = match (left, clip) {
                        (true, 1) => "shoot_leftlast".to_string(),
                        (false, 0) => "shoot_rightlast".to_string(),
                        (true, _) => format!("shoot_left{}", 1 + (frame.dice.roll() * 5.0) as u32 % 5),
                        (false, _) => format!("shoot_right{}", 1 + (frame.dice.roll() * 5.0) as u32 % 5),
                    };
                    Pick::Names(vec![n])
                }
                _ if empty => names(&["shootlast", "shoot_empty"], s),
                _ => names(&["shoot1", "shoot2", "shoot3", "shoot"], s),
            }
        }
        WeaponEventKind::ReloadStarted if s.shells => names(&["start_reload"], s),
        WeaponEventKind::ReloadStarted if short(s) == "glock18" => names(&["reload", "reload2"], s),
        WeaponEventKind::ReloadStarted => names(&["reload"], s),
        WeaponEventKind::ShellInserting => names(&["insert"], s),
        WeaponEventKind::Reloaded if s.shells => names(&["after_reload"], s),
        WeaponEventKind::ModeChanged { mode } if s.data.and_then(|d| d.alt.as_ref()).is_some_and(|a| matches!(a, AltData::Silencer { .. })) => {
            Pick::Names(vec![if *mode == 1 { "add_silencer" } else { "detach_silencer" }.to_string()])
        }
        WeaponEventKind::Swing { hit, secondary: true, .. } => Pick::Names(vec![if *hit { "stab" } else { "stab_miss" }.to_string()]),
        WeaponEventKind::Swing { secondary: false, .. } => {
            frame.dice.series = frame.dice.series.wrapping_add(1);
            Pick::Names(vec![if frame.dice.series % 2 == 1 { "midslash1" } else { "midslash2" }.to_string()])
        }
        WeaponEventKind::PinPulled => names(&["pullpin"], s),
        WeaponEventKind::Thrown => names(&["throw"], s),
        _ => return,
    };
    let fired = matches!(kind, WeaponEventKind::Fired { .. });
    let played = frame.attack(&pick);
    if fired {
        frame.fired = played;
        if frame.state.burst.is_some() {
            frame.dice.burst_started = frame.now;
        }
    }
}

/// What an animation event shows (module doc).
fn effect(e: &AnimEvent) -> Option<ViewModelEventKind> {
    use goldsrc_engine::content::weapon_models::{MUZZLE_EVENTS, flash_kind};
    if let Some(attachment) = MUZZLE_EVENTS.iter().position(|n| *n == e.event) {
        return Some(ViewModelEventKind::MuzzleFlashOf { attachment, kind: flash_kind(super::super::GAME, &e.options) });
    }
    match e.event {
        5004 if !e.options.trim().is_empty() => Some(ViewModelEventKind::Sound { entry: e.options.trim().to_lowercase() }),
        _ => None,
    }
}
