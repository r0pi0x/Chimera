//! Fitted parts on the first-person view model (specs/combat_arms/
//! attachments.md "View model and world model"): for each socket slot
//! whose part has a `Socket` and a model for the weapon's variant index
//! (`attachments::CaMods::part_models`; a default sight with
//! `DefaultScopeVisible` 0 isn't drawn), the part model `AttachModel`c
//! with its skins (`AttachSkin`c`_`i, by texture index), render styles
//! (`AttachRenderStyle#`) and `AttachScale`, at that socket of the gun,
//! moved by the weapon's `MuzzlePVPosOff`/`RotOff` (muzzle parts) or
//! `ScopePVPosOff`/`RotOff` (sights) in the socket's frame. The part
//! follows the socket's node (its vertices skinned rigidly to it), so it
//! moves, hides and shows with the gun.
//!
//! A weapon with drawn parts shows a view model of its own: the key
//! `<weapon id>+<part>+<part>` (`view_key`), made on demand
//! (`on_demand::load_weapon`) from the weapon's view model with the parts'
//! meshes added (`attach`); until it is ready the bare gun's stands in.
//!
//! The part sits in the socket's own frame (`first_person_frame`: the
//! socket's axes, the offset's angles added to the socket's; no turn of
//! our own), its model in the pose of its first animation
//! (`view_model::load_part`); attachments.md Corrections (2026-10-30)
//! 1 and 5.
//!
//! A part never moves the muzzle flash (Corrections 3); part models play
//! no animations and their sockets are unused (Corrections 5).

use bevy::prelude::*;

use super::{VIEW_SCALE, load_part};
use super::super::{
    attachments::{AttachParams, CaMods, CaModTable, ModRecord, PartOffset},
    render_style,
};
use crate::map::{MapData, MapViewModel};

/// What separates a weapon id from its parts in a view-model key.
pub const SEP: char = '+';

/// The view model to show for weapon `id` with fitted parts `mods`: its
/// own key, or `id+Part+Part` when parts with first-person models are
/// drawn on it.
pub fn view_key(id: &str, mods: Option<&CaMods>) -> String {
    let mut key = id.to_string();
    let Some(mods) = mods else { return key };
    for m in mods.part_models().into_iter().filter(|m| m.visible) {
        let has_model = mods.part(m.slot).is_some_and(|p| p.record.view_model(m.variant).is_some());
        if has_model {
            key.push(SEP);
            key.push_str(&m.name);
        }
    }
    key
}

/// A view-model key's weapon id and part names (its hands part,
/// `hands::split_key`, left out).
pub fn split_key(key: &str) -> (&str, Vec<&str>) {
    let key = super::hands::split_key(key).0;
    let mut it = key.split(SEP);
    let id = it.next().unwrap_or(key);
    (id, it.collect())
}

/// One part to put on a view model: its record, the weapon's variant
/// index for it, its slot's offset.
pub struct PartToDraw<'a> {
    pub record: &'a ModRecord,
    pub variant: usize,
    pub offset: PartOffset,
}

/// The parts of view-model key `key` as the weapon's data gives them:
/// each a known record with a socket, a variant in 0..9 from the weapon's
/// list and a first-person model there; unknown names are skipped.
pub fn parts_of<'a>(
    key: &str,
    table: &'a CaModTable,
    params: &AttachParams,
) -> Vec<PartToDraw<'a>> {
    let (_, names) = split_key(key);
    names
        .into_iter()
        .filter_map(|n| {
            let record = table.get(table.find(n)?)?;
            let variant = usize::try_from(params.variant_of(&record.name)?).ok().filter(|v| *v < 10)?;
            record.view_model(variant)?;
            let slot = record.slot()?;
            Some(PartToDraw {
                record,
                variant,
                offset: params.part_offsets.of(slot, true),
            })
        })
        .collect()
}

/// The socket transform `offset` adds in the third person, in the
/// socket's frame (the game's axes converted, units): the position, then
/// the rotation built from (pitch = its y, yaw = its x, roll = its z),
/// composed after the socket's rotation (attachments.md Corrections
/// (2026-10-30) 2).
pub fn offset_matrix(offset: &PartOffset) -> Mat4 {
    let pos = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset.pos));
    // Z mirrored: turns about x and y change sign, about z keep it.
    let [yaw, pitch, roll] = offset.rot;
    let rot = Quat::from_euler(EulerRot::YXZ, -yaw, -pitch, roll);
    Mat4::from_rotation_translation(rot, pos)
}

/// A first-person part's frame (attachments.md Corrections (2026-10-30)
/// 1): the socket's world transform `socket` moved along the socket's
/// own axes by the offset's position, its rotation the socket's split
/// into yaw, pitch and roll with the offset's angles (radians: x yaw, y
/// pitch, z roll) added (`added_angles`); no other turn.
pub fn first_person_frame(socket: Mat4, offset: &PartOffset) -> Mat4 {
    let (scale, rot, at) = socket.to_scale_rotation_translation();
    let pos = at + rot * (scale * Vec3::from(lithtech_formats::ltb::to_rh_vec(offset.pos)));
    Mat4::from_scale_rotation_translation(scale, added_angles(rot, offset.rot), pos)
}

/// Rotation `socket` (right-handed) with the game's angles `rot` (x yaw
/// about up, y pitch about right, z roll about forward, radians, in the
/// game's left-handed frame) added to its own yaw, pitch and roll and
/// rebuilt in the same order. Z mirrored: yaw and pitch change sign.
pub fn added_angles(socket: Quat, rot: [f32; 3]) -> Quat {
    let (yaw, pitch, roll) = socket.to_euler(EulerRot::YXZ);
    Quat::from_euler(EulerRot::YXZ, yaw - rot[0], pitch - rot[1], roll + rot[2])
}

/// Put `part`'s model (read through `file`) onto `view` at its socket,
/// its textures added to `textures`. Err: no such socket on the gun, or
/// the model doesn't read.
pub fn attach(
    view: &mut MapViewModel,
    part: &PartToDraw,
    textures: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<(), String> {
    let record = part.record;
    let socket_name = record.socket.as_deref().unwrap_or_default();
    let socket = view
        .attachments
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(socket_name))
        .ok_or_else(|| format!("{}: the gun has no `{socket_name}` socket", record.name))?
        .clone();
    let (path, skins) = record
        .view_model(part.variant)
        .ok_or_else(|| format!("{}: no model for variant {}", record.name, part.variant))?;
    let (mut meshes, lods) = load_part(path, skins, textures, file)?;
    // The socket's frame at the reference pose, in the eye's space.
    let globals = super::hands::globals(&view.bones);
    let bone = globals.get(socket.bone).copied().unwrap_or(Mat4::IDENTITY);
    let scale = Mat4::from_scale(Vec3::from(record.attach_scale.unwrap_or([1.0; 3])));
    let socket_m = view.root.to_matrix() * bone * socket.local.to_matrix();
    let to_eye = first_person_frame(socket_m, &part.offset) * scale;
    for (mesh, style) in meshes.iter_mut().zip(lods) {
        render_style::apply(mesh, render_style::of_list(&record.attach_render_styles, style));
        for p in &mut mesh.positions {
            *p = to_eye.transform_point3(Vec3::from(*p) / VIEW_SCALE).to_array();
        }
        for n in &mut mesh.normals {
            *n = to_eye.transform_vector3(Vec3::from(*n)).normalize_or_zero().to_array();
        }
        let count = mesh.positions.len();
        mesh.joints = vec![[socket.bone as u16, 0, 0, 0]; count];
        mesh.joint_weights = vec![[1.0, 0.0, 0.0, 0.0]; count];
        view.model.meshes.push(mesh.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_split_into_the_weapon_and_its_parts() {
        assert_eq!(split_key("combat_arms:m4a1"), ("combat_arms:m4a1", vec![]));
        assert_eq!(
            split_key("combat_arms:m4a1+Silencer_A_AR+DotSight_A_AR"),
            ("combat_arms:m4a1", vec!["Silencer_A_AR", "DotSight_A_AR"])
        );
    }

    #[test]
    fn a_zero_offset_is_the_socket_itself() {
        assert_eq!(offset_matrix(&PartOffset::default()), Mat4::IDENTITY);
    }
}
