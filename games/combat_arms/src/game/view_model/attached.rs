//! The view model's attached effects (specs/combat_arms/view_models.md 3,
//! case 44): a weapon's `PVAttachFXName0..` groups (`CaViewParams::
//! attach_fx`: the chainsaws' idle, the PSY bomb's fire, the time bombs'
//! lights) are made with the view model, hidden, as looping effects on it;
//! the machine (`machine::Pieces`) shows them when the model becomes
//! visible, hides them with it (a scope, the holster, death) and by the
//! `HIDE_PVATTACHFX` / `SHOW_PVATTACHFX` keys.
//!
//! `track` (after the view model's `drive`) keeps each character's
//! `CaAttachedFx`: the groups of the weapon drawn, each shown or not and
//! made when (its age drives the looping keys' curves); a new draw makes
//! them afresh. `sounds` plays the `PlaySound` keys of the local player's
//! shown groups as long-lived looping sounds (`CaLoopSound`, played by
//! `effects::sounds::loops`): started when the group shows, stopped when it
//! hides or the weapon goes. The front end draws the groups' keys
//! (`ui::effects`, camera-relative, at their sockets of the view model).
//!
//! Known shortcuts (docs/tech-debt.md): only looping `PlaySound` keys
//! play (all the stock attached groups' are); the sound plays only while its
//! group shows (whether a hidden group's sound key keeps sounding isn't
//! said: Open question in docs/plans/active/combat-arms.md), from the
//! player, local when the key says so, else 3D at the player with the
//! key's radii; others' view models (spectating) play no attached sounds;
//! the third-person (held) model has no attached effects (not specified).

use bevy::prelude::*;

use super::{CaViewModel, CaViewParams};
use crate::{core::LocalPlayer, map::SoundKey, weapon::Inventory};

/// One attached effect of the drawn view model.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedFx {
    /// The effect group (`PVAttachFXName`i).
    pub group: String,
    /// Shown now (3).
    pub shown: bool,
    /// When it was made (the draw; seconds of the clock).
    pub made: f64,
}

/// A character's view-model attached effects (none without a view model).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaAttachedFx {
    /// The weapon whose model they are on.
    pub weapon: Option<Entity>,
    pub effects: Vec<AttachedFx>,
}

impl CaAttachedFx {
    /// The groups shown now.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &AttachedFx)> {
        self.effects.iter().enumerate().filter(|(_, e)| e.shown)
    }
}

/// Keep each character's attached effects with its view model.
pub fn track(
    time: Res<Time>,
    mut characters: Query<(Entity, &CaViewModel, Option<&mut CaAttachedFx>)>,
    params: Query<&CaViewParams>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for (e, vm, attached) in &mut characters {
        let Some(mut attached) = attached else {
            commands.entity(e).insert(CaAttachedFx::default());
            continue;
        };
        // The model exists while the machine does (made at the draw,
        // destroyed with the holster).
        let weapon = vm.machine.as_ref().and(vm.weapon);
        if attached.weapon != weapon {
            let groups = weapon.and_then(|w| params.get(w).ok()).map_or(&[][..], |p| &p.attach_fx[..]);
            *attached = CaAttachedFx {
                weapon,
                effects: groups
                    .iter()
                    .map(|g| AttachedFx {
                        group: g.clone(),
                        shown: false,
                        made: now,
                    })
                    .collect(),
            };
        }
        let shown = vm.machine.as_ref().map_or(&[][..], |m| &m.pieces.fx_shown[..]);
        for (i, fx) in attached.effects.iter_mut().enumerate() {
            let s = shown.get(i).copied().unwrap_or(false);
            if fx.shown != s {
                fx.shown = s;
            }
        }
    }
}

/// Keys of the attached effects' looping sounds: this bit, the player's
/// entity and the effect's index.
pub const ATTACHED_SOUND_KEYS: u64 = 1 << 58;

/// The long-lived sound key of `owner`'s attached effect `index`, key `k`
/// of its group.
pub fn sound_key(owner: Entity, index: usize, k: usize) -> SoundKey {
    let id = (owner.to_bits() & 0xffff_ffff) << 16 | ((index as u64 & 0xff) << 8) | (k as u64 & 0xff);
    SoundKey(ATTACHED_SOUND_KEYS | id)
}

/// A `PlaySound` key of an effect group as a looping sound: the file, local
/// or 3D, its radii (u) and volume (0..100).
#[derive(Clone, Debug, PartialEq)]
pub struct GroupSound {
    pub key: usize,
    pub file: String,
    pub local: bool,
    pub radii: (f32, f32),
    pub volume: f32,
}

/// The looping `PlaySound` keys of `group` in the effect file (`lib`;
/// every stock attached group's sound loops).
pub fn group_sounds(lib: &crate::game::effects::library::FxLibrary, group: &str) -> Vec<GroupSound> {
    let Some(g) = lib.group(group) else { return Vec::new() };
    g.keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.kind.eq_ignore_ascii_case("PlaySound") && k.combo("Loop") == Some(1))
        .filter_map(|(i, k)| {
            let file = k.path("Sound").or_else(|| k.string("Sound"))?;
            if file.is_empty() {
                return None;
            }
            Some(GroupSound {
                key: i,
                file: crate::game::effects::library::normalize(file),
                local: k.combo("PlayLocal") == Some(1),
                radii: (
                    k.float("InnerRadius").unwrap_or(100.0),
                    k.float("OuterRadius").unwrap_or(500.0),
                ),
                volume: k.float("Volume").unwrap_or(100.0),
            })
        })
        .collect()
}

/// A looping sound to start (`Some`) or stop (`None`) under `key`.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaLoopSound {
    pub key: SoundKey,
    pub sound: Option<crate::game::effects::sounds::CaSound>,
    /// Plays from this entity while it exists.
    pub follow: Option<Entity>,
    /// Plays once, not looping (cut by a stop under its key all the same).
    pub once: bool,
}

/// The local player's shown attached effects' sounds: started when a group
/// shows, stopped when it hides or goes.
pub fn sounds(
    players: Query<(Entity, &CaAttachedFx, Option<&Transform>), With<LocalPlayer>>,
    mut playing: Local<Vec<(SoundKey, Entity)>>,
    mut out: MessageWriter<CaLoopSound>,
) {
    let lib = crate::game::effects::library::library();
    let mut wanted: Vec<(SoundKey, Entity, crate::game::effects::sounds::CaSound)> = Vec::new();
    if let Some(lib) = lib.as_deref() {
        for (e, attached, t) in &players {
            for (i, fx) in attached.shown() {
                for s in group_sounds(lib, &fx.group) {
                    let at = t.map_or(Vec3::ZERO, |t| t.translation);
                    let sound = if s.local {
                        crate::game::effects::sounds::CaSound::local(s.file.clone())
                    } else {
                        crate::game::effects::sounds::CaSound {
                            radii: s.radii,
                            ..crate::game::effects::sounds::CaSound::at(s.file.clone(), at, s.radii.1)
                        }
                    };
                    wanted.push((sound_key(e, i, s.key), e, sound.volume(s.volume)));
                }
            }
        }
    }
    playing.retain(|(k, _)| {
        let keep = wanted.iter().any(|(w, ..)| w == k);
        if !keep {
            out.write(CaLoopSound {
                key: *k,
                sound: None,
                follow: None,
                once: false,
            });
        }
        keep
    });
    for (k, e, s) in wanted {
        if playing.iter().any(|(p, _)| *p == k) {
            continue;
        }
        playing.push((k, e));
        out.write(CaLoopSound {
            key: k,
            sound: Some(s),
            follow: Some(e),
            once: false,
        });
    }
}

/// The attached effects of `inventory`'s weapon in hand, if `attached` is on
/// it.
pub fn of_active<'a>(attached: &'a CaAttachedFx, inventory: &Inventory) -> Option<&'a CaAttachedFx> {
    (attached.weapon.is_some() && attached.weapon == inventory.active).then_some(attached)
}

/// Where a key of an attached effect sits, in the view-model camera's
/// space (metres, Bevy axes), posed as drawn: a key attached to a socket
/// or node of the view model (`UpdatePos` 3-6 with `AttachName`; sockets
/// first, then nodes, names without case) is there plus its `Offset` in
/// that socket's axes (units); any other (fixed, follow, player view) and
/// a name the model lacks sit at the model's origin plus the offset.
pub fn key_place(
    model: &crate::map::MapViewModel,
    pose: &[crate::map::anim::BonePose],
    offset: &crate::map::ViewModelOffset,
    attach: Option<&str>,
    key_offset: Vec3,
) -> Mat4 {
    let placed = crate::map::view_model::placement(offset, false);
    let o = Vec3::new(key_offset.x, key_offset.y, -key_offset.z);
    let name = attach.filter(|n| !n.is_empty());
    let socket = name.and_then(|n| model.attachments.iter().position(|a| a.name.eq_ignore_ascii_case(n)));
    let base = match socket {
        Some(i) => crate::map::view_model::attachment_transform(model, pose, i, &placed),
        None => name.and_then(|n| model.bones.iter().position(|b| b.name.eq_ignore_ascii_case(n))).map(|bone| {
            let mut global: Vec<Mat4> = Vec::with_capacity(model.bones.len());
            for (i, b) in model.bones.iter().enumerate() {
                let (q, p) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
                let local = Mat4::from_rotation_translation(q, p);
                global.push(b.parent.and_then(|p| global.get(p)).map_or(local, |parent| *parent * local));
            }
            placed.to_matrix() * model.root.to_matrix() * global[bone]
        }),
    }
    .unwrap_or_else(|| placed.to_matrix() * model.root.to_matrix());
    base * Mat4::from_translation(o)
}
