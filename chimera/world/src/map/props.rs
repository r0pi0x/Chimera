//! Placed props after spawning: their animated sequences (and
//! `PropAnimationDone` when one ends, for the logic's OnAnimationDone; a
//! game holding one still at a cycle with `PropCycle`) and the
//! components that tag them (their looks as drawn: `render::props`;
//! their dynamic shadows: `render::shadows`). `posed_model`: a rigged
//! model's meshes moved to one frame of a sequence, for drawing it still
//! where no skeleton is (a game's shown items).

#[allow(unused_imports)]
use super::*;

/// A simulated physics prop: how players interact with it, its mass (kg)
/// and its model-space bounds.
#[derive(Component, Debug, Clone, Copy)]
pub struct PhysicsProp {
    pub push: PushAway,
    pub mass: f32,
    pub bounds: (Vec3, Vec3),
}

/// A prop entity's index in `MapData::props`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PropIndex(pub usize);

/// The map entity a prop node was placed by (index into
/// `MapData::entities`), so logic can find it (a sound playing from it).
#[derive(Component, Debug, Clone, Copy)]
pub struct PropEntity(pub usize);

/// Where an entity prop was placed (its spawn transform), so a round
/// restart can put a moved physics prop back.
#[derive(Component, Debug, Clone, Copy)]
pub struct PropHome(pub Transform);

/// An animated prop: what it plays, its joints by bone, the logic's last
/// word and the sequence it falls back to.
#[derive(Component)]
pub struct PropRig {
    pub animator: anim::Animator,
    pub joints: Vec<Entity>,
    pub seen: Option<PropSequence>,
    pub default: Option<usize>,
    /// The sequence that ended was reported (`PropAnimationDone`); until it
    /// restarts.
    pub done: bool,
}

/// An animated prop's non-looping sequence reached its end (the logic
/// fires OnAnimationDone for the prop on `node`). `serial` is the logic's
/// serial of the sequence that played (0: the spawn-time default); a
/// default animation that restarts reports at each pass.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropAnimationDone {
    pub node: Entity,
    pub serial: u32,
}

/// Inverse bind poses of a rig's bones in the reference pose under its
/// root.
pub fn rig_inverse_bindposes(rig: &MapRig) -> bevy::mesh::skinning::SkinnedMeshInverseBindposes {
    let mut global: Vec<Mat4> = Vec::with_capacity(rig.bones.len());
    for b in &rig.bones {
        let local = Mat4::from_rotation_translation(b.rotation, b.position);
        let parent = b
            .parent
            .and_then(|p| global.get(p).copied())
            .unwrap_or(rig.root.to_matrix());
        global.push(parent * local);
    }
    let inverse: Vec<Mat4> = global.iter().map(|m| m.inverse()).collect();
    bevy::mesh::skinning::SkinnedMeshInverseBindposes::from(inverse)
}

/// Each bone's model-space matrix (engine axes, meters) for local poses
/// `pose` of `rig`.
fn rig_globals(rig: &MapRig, pose: &[anim::BonePose]) -> Vec<Mat4> {
    let mut global: Vec<Mat4> = Vec::with_capacity(rig.bones.len());
    for (i, b) in rig.bones.iter().enumerate() {
        let (q, p) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
        let local = Mat4::from_rotation_translation(q, p);
        let parent = b
            .parent
            .and_then(|p| global.get(p).copied())
            .unwrap_or(rig.root.to_matrix());
        global.push(parent * local);
    }
    global
}

/// `model` with its skinned meshes moved to sequence `sequence` at `cycle`
/// (0..1), as one still model (no rig, no levels of detail); None when it
/// has no rig or no such sequence.
pub fn posed_model(model: &MapModel, sequence: &str, cycle: f32) -> Option<MapModel> {
    let rig = model.rig.as_ref()?;
    let set = &rig.animations;
    let s = set.sequence(sequence)?;
    let mut pose: Vec<anim::BonePose> = rig.bones.iter().map(|b| (b.rotation, b.position)).collect();
    set.sequence_pose(s, cycle, &set.default_params(), &mut pose);
    let rest = rig_globals(rig, &[]);
    let posed = rig_globals(rig, &pose);
    let skin: Vec<Mat4> = posed.iter().zip(&rest).map(|(p, r)| *p * r.inverse()).collect();
    let mut out = model.clone();
    out.rig = None;
    out.lods.clear();
    for m in &mut out.meshes {
        if m.joints.len() != m.positions.len() {
            continue;
        }
        for (i, (pos, nrm)) in m.positions.iter_mut().zip(m.normals.iter_mut()).enumerate() {
            let (j, w) = (m.joints[i], m.joint_weights[i]);
            let mut mat = Mat4::ZERO;
            for k in 0..4 {
                if w[k] > 0.0
                    && let Some(b) = skin.get(j[k] as usize)
                {
                    mat += *b * w[k];
                }
            }
            if mat == Mat4::ZERO {
                continue;
            }
            *pos = mat.transform_point3(Vec3::from_array(*pos)).to_array();
            *nrm = mat.transform_vector3(Vec3::from_array(*nrm)).normalize_or_zero().to_array();
        }
    }
    Some(out)
}

/// Holds an animated prop's sequence still at this cycle (0..1): a game's
/// gauge drawn by its model's frame (HL2's wall chargers' dial). Without
/// it the prop plays on.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PropCycle(pub f32);

/// Animated props play what the logic asks (`PropSequence`,
/// specs/source/prop_damage.md 9): a new serial restarts the named
/// sequence (sequence 0 when the model lacks it), switching at once; a
/// new round starts the default one (else the first); a non-looping
/// sequence that ends starts the default again, if there is one.
pub fn animate_props(
    time: Res<Time>,
    mut props: Query<(Entity, &mut PropRig, Option<&PropSequence>, Option<&PropCycle>)>,
    mut done: MessageWriter<PropAnimationDone>,
    mut joints: Query<&mut Transform>,
) {
    let (now, dt) = (time.elapsed_secs_f64(), time.delta_secs());
    for (node, mut rig, seq, held) in &mut props {
        let rig = &mut *rig;
        if let Some(seq) = seq
            && rig.seen.as_ref() != Some(seq)
        {
            let old = rig.seen.replace(seq.clone());
            rig.default = Some(seq.default.as_str())
                .filter(|d| !d.is_empty())
                .and_then(|d| rig.animator.set.sequence(d));
            let new_round = old.as_ref().is_none_or(|o| o.round != seq.round);
            let old_serial = old.as_ref().and_then(|o| o.play.as_ref()).map(|p| p.1);
            match &seq.play {
                Some((name, serial)) if new_round || old_serial != Some(*serial) => {
                    let s = rig.animator.set.sequence(name).unwrap_or_else(|| {
                        warn!("prop: no sequence {name}");
                        0
                    });
                    rig.animator.restart(s, now);
                    rig.animator.fading.clear();
                    rig.done = false;
                }
                // No default: its first sequence (the model's idle).
                None if new_round => {
                    rig.animator.restart(rig.default.unwrap_or(0), now);
                    rig.animator.fading.clear();
                    rig.done = false;
                }
                _ => {}
            }
        }
        if let Some(PropCycle(c)) = held {
            // Held still: its sequence (the default's, else the first) at
            // that frame.
            if rig.animator.main.is_none() {
                rig.animator.restart(rig.default.unwrap_or(0), now);
            }
            rig.animator.fading.clear();
            rig.animator.cycle = c.clamp(0.0, 1.0);
            let pose = rig.animator.pose(now);
            for (joint, (q, p)) in rig.joints.iter().zip(pose) {
                if let Ok(mut t) = joints.get_mut(*joint) {
                    t.rotation = q;
                    t.translation = p;
                }
            }
            continue;
        }
        if rig.animator.main.is_none() {
            continue;
        }
        rig.animator.advance(dt, now);
        // The end of a non-looping sequence is reported once (the logic's
        // OnAnimationDone); a non-looping default restarts too, forever,
        // reporting each pass.
        if rig.animator.finished() {
            if !rig.done {
                rig.done = true;
                let serial = rig.seen.as_ref().and_then(|s| s.play.as_ref()).map_or(0, |p| p.1);
                done.write(PropAnimationDone { node, serial });
            }
            if let Some(d) = rig.default {
                rig.animator.restart(d, now);
                rig.animator.fading.clear();
                rig.done = false;
            }
        }
        let pose = rig.animator.pose(now);
        for (joint, (q, p)) in rig.joints.iter().zip(pose) {
            if let Ok(mut t) = joints.get_mut(*joint) {
                t.rotation = q;
                t.translation = p;
            }
        }
    }
}

/// A prop's physics collider (sprite glows see through static props, as
/// the game's line test ignores them).
#[derive(Component)]
pub struct MapPropCollider;

/// A map collider light passes (glass, grates: `MapData::see_through_hulls`
/// and `see_through_entities`): solid to players and shots, not to a
/// flash's line (grenades.md Q12).
#[derive(Component)]
pub struct SeeThrough;

#[cfg(test)]
mod switch_tests {
    use super::*;

    #[test]
    fn body_numbers_pick_one_choice_per_part() {
        // Parts with 1, 3 and 2 choices: body = c0 + 1 * c1 + 3 * c2.
        let parts = [1, 3, 2];
        assert_eq!(body_choice(&parts, 5, 1), 2);
        assert_eq!(body_choice(&parts, 5, 2), 1);
    }

    #[test]
    fn light_styles_add_and_remove_their_share() {
        let l = MapLightmap {
            width: 2,
            height: 1,
            rgb: vec![[1.0; 3], [0.5; 3]],
            bumped: None,
            styles: vec![
                MapLightStyle {
                    style: 32,
                    on: true,
                    texels: vec![0],
                    rgb: vec![[0.75; 3]],
                    ..default()
                },
                MapLightStyle {
                    style: 33,
                    on: false,
                    texels: vec![1],
                    rgb: vec![[0.25; 3]],
                    ..default()
                },
            ],
        };
        let lit = |on: u8| move |s: &MapLightStyle| if s.style == on { 1.0 } else { 0.0 };
        assert_eq!(l.relit(&lit(32)).0, l.rgb);
        assert_eq!(l.relit(&lit(33)).0, vec![[0.25; 3], [0.75; 3]]);
    }
}
