//! What Source view models play (lift S8, docs/plans/active/hl2dm.md;
//! moved from CS:S's `game::view_anim`): the shared weapon base's
//! activities by name (`ACT_VM_*`, specs/cs_source/weapons.md 3.3-3.8,
//! specs/hl2dm/weapons.md 2, 7), the effects of the models' animation
//! events (`event_effect`: muzzle flashes and brass, specs/cs_source/
//! view_models.md 6-8), and the reactions every Source game shares
//! (`common_event`: a reload, a shell going in, a shell reload's finish,
//! a grenade's pin and throw, the bomb's key presses). Each game's
//! `weapon::view_anim::ViewAnimRules` choose the rest (CS:S's knife,
//! silencers and dual pistols; HL2:DM's shotgun pump and double shot) and
//! call these.

use crate::{
    map::{ViewModelEventKind, anim::AnimEvent},
    weapon::{
        WeaponEventKind,
        view_anim::{Pick, ViewFrame},
    },
};

pub const DRAW: &str = "ACT_VM_DRAW";
pub const IDLE: &str = "ACT_VM_IDLE";
pub const PRIMARY: &str = "ACT_VM_PRIMARYATTACK";
pub const SECONDARY: &str = "ACT_VM_SECONDARYATTACK";
pub const RELOAD: &str = "ACT_VM_RELOAD";
pub const DRYFIRE: &str = "ACT_VM_DRYFIRE";
/// Dual pistols: the last round from either hand.
pub const DRYFIRE_LEFT: &str = "ACT_VM_DRYFIRE_LEFT";
/// Shotguns (spec weapons.md 3.4): the reload's start and finish; each
/// shell going in plays `RELOAD`; HL2's pump after a shot.
pub const SHOTGUN_RELOAD_START: &str = "ACT_SHOTGUN_RELOAD_START";
pub const SHOTGUN_RELOAD_FINISH: &str = "ACT_SHOTGUN_RELOAD_FINISH";
pub const SHOTGUN_PUMP: &str = "ACT_SHOTGUN_PUMP";
pub const ATTACH_SILENCER: &str = "ACT_VM_ATTACH_SILENCER";
pub const DETACH_SILENCER: &str = "ACT_VM_DETACH_SILENCER";
pub const HIT_CENTER: &str = "ACT_VM_HITCENTER";
/// Grenades (spec grenades.md 2): the pin pull (the unused pull variants
/// are the same sequence) and the throw.
pub const PULL_PIN: &str = "ACT_VM_PULLPIN";
pub const PULL_BACK: &str = "ACT_VM_PULLBACK_HIGH";
pub const THROW: &str = "ACT_VM_THROW";
pub const MISS_CENTER: &str = "ACT_VM_MISSCENTER";

/// Model event numbers (spec view_models.md 6): a muzzle flash on the
/// attachment its option names (1-based); the named client effect that
/// ejects brass ("EjectBrass_<type> <attachment> <speed>").
const EVENT_MUZZLE_FLASHES: &[i32] = &[5001, 5011, 5021, 5031];
const EVENT_CLIENT_EFFECT_ATTACH: &str = "AE_CLIENT_EFFECT_ATTACH";

/// What a view-model animation event asks the map to show.
pub fn event_effect(e: &AnimEvent) -> Option<ViewModelEventKind> {
    if EVENT_MUZZLE_FLASHES.contains(&e.event) {
        let n: usize = e.options.trim().parse().ok()?;
        return Some(ViewModelEventKind::MuzzleFlash {
            attachment: n.checked_sub(1)?,
        });
    }
    if e.name.eq_ignore_ascii_case(EVENT_CLIENT_EFFECT_ATTACH) {
        let mut words = e.options.split_whitespace();
        let effect = words.next()?;
        let shell = effect.strip_prefix("EjectBrass_")?;
        let attachment: usize = words.next()?.parse().ok()?;
        let speed: f32 = words.next().and_then(|w| w.parse().ok()).unwrap_or(0.0);
        return Some(ViewModelEventKind::EjectBrass {
            attachment: attachment.checked_sub(1)?,
            shell: shell.to_string(),
            speed,
        });
    }
    None
}

fn acts(list: &[&str]) -> Pick {
    Pick::activities(list)
}

/// The reactions every Source game's view models share; `reload` is the
/// magazine reload's pick (CS:S's silenced variants).
pub fn common_event_with(f: &mut ViewFrame, kind: &WeaponEventKind, reload: Pick) {
    match kind {
        WeaponEventKind::ReloadStarted => {
            let pick = if f.state.shells { acts(&[SHOTGUN_RELOAD_START]) } else { reload };
            f.attack(&pick);
        }
        WeaponEventKind::ShellInserting => {
            f.attack(&acts(&[RELOAD]));
        }
        WeaponEventKind::Reloaded if f.state.shells => {
            f.attack(&acts(&[SHOTGUN_RELOAD_FINISH]));
        }
        WeaponEventKind::PinPulled => {
            f.attack(&acts(&[PULL_PIN, PULL_BACK]));
        }
        WeaponEventKind::Thrown => {
            f.attack(&acts(&[THROW]));
        }
        // The C4's key presses (spec objectives.md: `pressbutton`, the
        // primary activity); stopping short goes back to idle.
        WeaponEventKind::ArmingStarted => {
            f.attack(&acts(&[PRIMARY]));
        }
        WeaponEventKind::ArmingStopped => {
            f.play(&acts(&[IDLE]));
        }
        _ => {}
    }
}

/// `common_event_with` the plain reload.
pub fn common_event(f: &mut ViewFrame, kind: &WeaponEventKind) {
    common_event_with(f, kind, acts(&[RELOAD]));
}
