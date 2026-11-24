//! The MDL v10 reader on the install, against specs/goldsrc/mdl.md: gign
//! and v_ak47 as the worked examples (header, hitboxes, sequences, events,
//! animation streams, triangle commands), barney's external textures and
//! sequence group, and every version 10 file of `cstrike/` and `valve/`.

use goldsrc_formats::mdl::{Mdl, check_sequence_file};

use super::install::{files, mount};

fn model(path: &str) -> Option<Mdl> {
    let m = mount()?;
    Some(Mdl::parse(m.read(path).unwrap()).unwrap())
}

#[test]
fn gign() {
    let Some(g) = model("models/player/gign/gign.mdl") else {
        return;
    };
    assert_eq!(
        (g.length, g.bones.len(), g.hitboxes.len(), g.sequences.len()),
        (2_329_328, 53, 21, 111)
    );
    assert_eq!(
        (
            g.textures.len(),
            g.body_parts.len(),
            g.controllers.len(),
            g.attachments.len()
        ),
        (2, 2, 1, 2)
    );
    assert_eq!(g.bones[0].name, "Bip01");
    assert_eq!(g.bones[0].parent, -1);
    let h = &g.hitboxes[11];
    assert_eq!((g.bones[h.bone as usize].name.as_str(), h.group), ("Bip01 Head", 1));
    assert_eq!((h.mins, h.maxs), ([-0.33, -4.5, -2.5], [8.0, 4.5, 2.5]));
    assert_eq!(g.hitboxes[20].group, 8);
    let (_, run) = g.sequence("run").unwrap();
    assert_eq!(
        (run.fps, run.frames, run.looping(), run.motion_type),
        (60.0, 37, true, 0x40)
    );
    assert!((run.linear_movement[0] - 150.0).abs() < 1e-3 && run.linear_movement[1..] == [0.0, 0.0]);
    assert!((run.linear_movement[0] / run.duration() - 250.0).abs() < 1e-2);
    let (_, aim) = g.sequence("ref_aim_ak47").unwrap();
    assert_eq!(
        (aim.blends, aim.blend_type[0], aim.blend_start[0], aim.blend_end[0]),
        (9, 8, -90.0, 90.0)
    );
    assert_eq!(g.controllers[0].bone, 8);
    assert_eq!(g.controllers[0].kind, 32);
    assert_eq!(g.body_parts[1].submodels.len(), 2);
    assert!(g.body_parts[1].submodels[0].meshes.is_empty());
    let body = &g.body_parts[0].submodels[0];
    let tris: usize = body.meshes.iter().map(|m| m.triangles().len()).sum();
    assert_eq!(tris, 740);
}

#[test]
fn v_ak47() {
    let Some(v) = model("models/v_ak47.mdl") else { return };
    assert_eq!(
        (v.bones.len(), v.hitboxes.len(), v.sequences.len(), v.textures.len()),
        (42, 4, 6, 11)
    );
    assert_eq!(
        v.body_parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["rhand", "lhand", "weapon"]
    );
    let reload = &v.sequences[1];
    assert_eq!((reload.name.as_str(), reload.fps, reload.frames), ("reload", 37.0, 91));
    let ev: Vec<_> = reload
        .events
        .iter()
        .map(|e| (e.frame, e.event, e.options.as_str()))
        .collect();
    assert_eq!(
        ev,
        [
            (13, 5004, "weapons/ak47_clipout.wav"),
            (57, 5004, "weapons/ak47_clipin.wav")
        ]
    );
    let x = v.channel_value(&v.bytes, 3, 0, 0, 0, 0).unwrap();
    assert!((x - 10.3748).abs() < 1e-3, "{x}");
    let ry = v.stored_values(&v.bytes, 1, 0, 5, 4).unwrap();
    assert_eq!([ry[27], ry[28], ry[57], ry[58], ry[59]], [0, 2, 167, 167, 167]);
    let weapon = &v.body_parts[2].submodels[0];
    let tris: usize = weapon.meshes.iter().map(|m| m.triangles().len()).sum();
    assert_eq!(tris, 454);
    assert_eq!(v.textures[0].name, "view_glove.bmp");
    assert_eq!(
        (v.textures[0].width, v.textures[0].height, v.textures[0].pixel_offset),
        (256, 128, 57_304)
    );
    assert_eq!(v.attachments[0].bone, 20);
}

#[test]
fn barney_has_external_textures_and_a_sequence_group() {
    let Some(m) = mount() else { return };
    let b = Mdl::parse(m.read("models/barney.mdl").unwrap()).unwrap();
    assert!(b.textures.is_empty());
    assert_eq!(b.groups.len(), 4);
    let s = &b.sequences[18];
    assert_eq!(
        (s.name.as_str(), s.group, s.anim_offset, s.frames),
        ("cower_stand", 1, 76, 51)
    );
    let file = m.read(&b.groups[1].file.replace('\\', "/")).unwrap();
    check_sequence_file(&file).unwrap();
    assert_eq!(file.len(), 64_668);
    for bone in 0..b.bones.len() {
        b.stored_values(&file, 18, 0, bone, 0).unwrap();
    }
    let t = Mdl::parse(m.read("models/barneyt.mdl").unwrap()).unwrap();
    assert_eq!(t.textures.len(), 26);
}

#[test]
fn every_version_10_model_reads() {
    let all: Vec<_> = files("cstrike/models", "mdl")
        .into_iter()
        .chain(files("valve/models", "mdl"))
        .collect();
    if all.is_empty() {
        return;
    }
    let (mut models, mut sequence_files) = (0, 0);
    for path in &all {
        let bytes = std::fs::read(path).unwrap();
        match &bytes[..4] {
            b"IDSQ" => {
                check_sequence_file(&bytes).unwrap();
                sequence_files += 1;
            }
            _ if i32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 10 => {}
            _ => {
                let m = Mdl::parse(bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                for (si, s) in m.sequences.iter().enumerate().filter(|(_, s)| s.group == 0) {
                    for blend in 0..s.blends.max(1) as usize {
                        for bone in 0..m.bones.len() {
                            for ch in 0..6 {
                                m.stored_values(&m.bytes, si, blend, bone, ch)
                                    .unwrap_or_else(|e| panic!("{} {}: {e}", path.display(), s.name));
                            }
                        }
                    }
                }
                models += 1;
            }
        }
    }
    assert!(
        models >= 466 && sequence_files >= 35,
        "{models} models, {sequence_files} sequence files"
    );
}
