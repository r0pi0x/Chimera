//! Studio models, MDL version 10 (specs/goldsrc/mdl.md): `Mdl::parse`
//! reads the header and every array (bones, bone controllers, hitboxes,
//! sequences with their events, sequence groups, textures with their
//! pixels and palettes, the skin table, body parts with their submodels,
//! meshes and triangle commands, attachments), checking each offset is in
//! the file. A model without textures has them in `<name>T.mdl` (read with
//! `Mdl::parse` too: no bones, the texture fields filled in); sequence
//! group files `<name>NN.mdl` (`IDSQ`) are checked with
//! `check_sequence_file` and hold the animation of groups 1 and up.
//!
//! Animation (`Mdl::stored_values`, `Mdl::channel_value`): for each
//! sequence, blend and bone a record of six channel offsets relative to
//! itself; a channel's stream is blocks of (valid, total) frames, valid
//! i16 values, the last repeated to the block's end; 0 is a constant
//! channel (the bone's default).

use chimera_vfs::bytes::Le;

use crate::{Result, err, fixed_name};

pub const MAGIC: &[u8; 4] = b"IDST";
pub const SEQUENCE_MAGIC: &[u8; 4] = b"IDSQ";
pub const VERSION: i32 = 10;

/// A bone: parent (−1 for a root), the controllers driving each channel,
/// each channel's default and the worth of one stored step (position x,
/// y, z, rotation x, y, z; radians for rotations).
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    pub flags: i32,
    pub controllers: [i32; 6],
    pub value: [f32; 6],
    pub scale: [f32; 6],
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneController {
    pub bone: i32,
    /// Channel bits: 1, 2, 4 position; 8, 16, 32 rotation; 0x8000 wraps.
    pub kind: i32,
    pub start: f32,
    pub end: f32,
    pub rest: i32,
    pub index: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hitbox {
    pub bone: i32,
    pub group: i32,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub frame: i32,
    pub event: i32,
    pub kind: i32,
    pub options: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub fps: f32,
    pub flags: i32,
    pub activity: i32,
    pub activity_weight: i32,
    pub events: Vec<Event>,
    pub frames: i32,
    pub motion_type: i32,
    pub motion_bone: i32,
    pub linear_movement: [f32; 3],
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub blends: i32,
    /// Into the main file for group 0, into the group's file otherwise.
    pub anim_offset: i32,
    pub blend_type: [i32; 2],
    pub blend_start: [f32; 2],
    pub blend_end: [f32; 2],
    pub group: i32,
}

impl Sequence {
    pub fn looping(&self) -> bool {
        self.flags & 1 != 0
    }

    /// One cycle's length in seconds: (frames − 1) / fps.
    pub fn duration(&self) -> f32 {
        if self.fps > 0.0 {
            (self.frames - 1).max(0) as f32 / self.fps
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceGroup {
    pub label: String,
    /// Mount-relative, backslashes as stored (`models\barney01.mdl`).
    pub file: String,
}

/// A texture: its pixels (indices) and 256-colour palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub name: String,
    /// 1 flat, 2 chrome, 4 fullbright, 32 additive, 64 masked.
    pub flags: i32,
    pub width: u32,
    pub height: u32,
    pub pixel_offset: u32,
    pub pixels: Vec<u8>,
    pub palette: Vec<[u8; 3]>,
}

impl Texture {
    /// As RGBA8: index 255 transparent when masked (flag 64), its colour
    /// filled from the opaque texels (`holes::fill`).
    pub fn rgba(&self) -> Vec<u8> {
        let masked = self.flags & 64 != 0;
        let mut out: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|&i| {
                let c = self.palette[i as usize];
                [c[0], c[1], c[2], if masked && i == 255 { 0 } else { 255 }]
            })
            .collect();
        if masked {
            crate::holes::fill(&mut out, self.width as usize, self.height as usize);
        }
        out
    }
}

/// One corner of a triangle command: vertex, normal, texel s and t.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriVertex {
    pub vertex: i16,
    pub normal: i16,
    pub s: i16,
    pub t: i16,
}

/// A run of a mesh's triangle commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriCommand {
    Strip(Vec<TriVertex>),
    Fan(Vec<TriVertex>),
}

impl TriCommand {
    /// The run's triangles as corner triples (a strip alternates winding).
    pub fn triangles(&self) -> Vec<[TriVertex; 3]> {
        match self {
            TriCommand::Strip(v) => (2..v.len())
                .map(|i| {
                    if i % 2 == 0 {
                        [v[i - 2], v[i - 1], v[i]]
                    } else {
                        [v[i - 1], v[i - 2], v[i]]
                    }
                })
                .collect(),
            TriCommand::Fan(v) => (2..v.len()).map(|i| [v[0], v[i - 1], v[i]]).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mesh {
    pub triangle_count: i32,
    pub skin_ref: i32,
    pub normal_count: i32,
    pub commands: Vec<TriCommand>,
}

impl Mesh {
    pub fn triangles(&self) -> Vec<[TriVertex; 3]> {
        self.commands.iter().flat_map(|c| c.triangles()).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Submodel {
    pub name: String,
    pub meshes: Vec<Mesh>,
    /// Each vertex's bone, and the vertex in that bone's frame.
    pub vertex_bones: Vec<u8>,
    pub vertices: Vec<[f32; 3]>,
    pub normal_bones: Vec<u8>,
    pub normals: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyPart {
    pub name: String,
    pub base: i32,
    pub submodels: Vec<Submodel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub name: String,
    pub bone: i32,
    pub position: [f32; 3],
}

/// A model (or a `<name>T.mdl` texture file).
#[derive(Clone, Debug)]
pub struct Mdl {
    pub bytes: Vec<u8>,
    pub name: String,
    pub length: i32,
    pub eye_position: [f32; 3],
    pub flags: i32,
    pub bones: Vec<Bone>,
    pub controllers: Vec<BoneController>,
    pub hitboxes: Vec<Hitbox>,
    pub sequences: Vec<Sequence>,
    pub groups: Vec<SequenceGroup>,
    pub textures: Vec<Texture>,
    /// skin families × references: texture indices.
    pub skins: Vec<Vec<i16>>,
    pub body_parts: Vec<BodyPart>,
    pub attachments: Vec<Attachment>,
}

/// An array's (count, offset) checked against the file: every record in
/// bounds.
fn array(bytes: &[u8], count: i32, offset: i32, size: usize, what: &str) -> Result<Vec<usize>> {
    if count < 0 || offset < 0 || offset as usize + count as usize * size > bytes.len() {
        return err(format!(
            "{what}: {count} records at {offset} run past the file ({} bytes)",
            bytes.len()
        ));
    }
    Ok((0..count as usize).map(|i| offset as usize + i * size).collect())
}

fn at(bytes: &[u8], pos: usize) -> Le<'_> {
    let mut r = Le::new(bytes);
    r.pos = pos.min(bytes.len());
    r
}

fn read_name(r: &mut Le, n: usize) -> Result<String> {
    Ok(fixed_name(r.take(n)?))
}

fn vec3s(bytes: &[u8], count: i32, offset: i32, what: &str) -> Result<Vec<[f32; 3]>> {
    array(bytes, count, offset, 12, what)?
        .into_iter()
        .map(|p| Ok(at(bytes, p).f32s::<3>()?))
        .collect()
}

fn read_commands(bytes: &[u8], offset: i32) -> Result<Vec<TriCommand>> {
    if offset < 0 {
        return err("triangle commands at a negative offset");
    }
    let mut r = at(bytes, offset as usize);
    let mut out = Vec::new();
    loop {
        let n = r.i16()?;
        if n == 0 {
            return Ok(out);
        }
        let v = (0..n.unsigned_abs())
            .map(|_| {
                Ok(TriVertex {
                    vertex: r.i16()?,
                    normal: r.i16()?,
                    s: r.i16()?,
                    t: r.i16()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        out.push(if n > 0 {
            TriCommand::Strip(v)
        } else {
            TriCommand::Fan(v)
        });
    }
}

impl Mdl {
    pub fn parse(bytes: Vec<u8>) -> Result<Mdl> {
        let b = &bytes[..];
        let mut r = Le::new(b);
        if r.take(4)? != MAGIC {
            return err("not a studio model (IDST)");
        }
        let version = r.i32()?;
        if version != VERSION {
            return err(format!("studio model version {version}, not {VERSION}"));
        }
        let model_name = read_name(&mut r, 64)?;
        let length = r.i32()?;
        let eye_position = r.f32s::<3>()?;
        r.take(48)?; // movement hull and clipping box
        let flags = r.i32()?;
        let mut pair = || -> Result<(i32, i32)> { Ok((r.i32()?, r.i32()?)) };
        let bones_at = pair()?;
        let ctl_at = pair()?;
        let hit_at = pair()?;
        let seq_at = pair()?;
        let grp_at = pair()?;
        let (tex_count, tex_offset) = pair()?;
        let tex_data = r.i32()?;
        let (skin_refs, skin_families) = (r.i32()?, r.i32()?);
        let skin_offset = r.i32()?;
        let body_at = {
            let mut r2 = at(b, 204);
            (r2.i32()?, r2.i32()?)
        };
        let att_at = {
            let mut r2 = at(b, 212);
            (r2.i32()?, r2.i32()?)
        };
        let _ = tex_data;

        let bones = array(b, bones_at.0, bones_at.1, 112, "bones")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                Ok(Bone {
                    name: read_name(&mut r, 32)?,
                    parent: r.i32()?,
                    flags: r.i32()?,
                    controllers: [r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?],
                    value: r.f32s::<6>()?,
                    scale: r.f32s::<6>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let controllers = array(b, ctl_at.0, ctl_at.1, 24, "bone controllers")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                Ok(BoneController {
                    bone: r.i32()?,
                    kind: r.i32()?,
                    start: r.f32()?,
                    end: r.f32()?,
                    rest: r.i32()?,
                    index: r.i32()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let hitboxes = array(b, hit_at.0, hit_at.1, 32, "hitboxes")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                Ok(Hitbox {
                    bone: r.i32()?,
                    group: r.i32()?,
                    mins: r.f32s::<3>()?,
                    maxs: r.f32s::<3>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let sequences = array(b, seq_at.0, seq_at.1, 176, "sequences")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                let name = read_name(&mut r, 32)?;
                let fps = r.f32()?;
                let flags = r.i32()?;
                let activity = r.i32()?;
                let activity_weight = r.i32()?;
                let (ev_count, ev_offset) = (r.i32()?, r.i32()?);
                let frames = r.i32()?;
                r.take(8)?; // pivots
                let motion_type = r.i32()?;
                let motion_bone = r.i32()?;
                let linear_movement = r.f32s::<3>()?;
                r.take(8)?;
                let mins = r.f32s::<3>()?;
                let maxs = r.f32s::<3>()?;
                let blends = r.i32()?;
                let anim_offset = r.i32()?;
                let blend_type = [r.i32()?, r.i32()?];
                let blend_start = [r.f32()?, r.f32()?];
                let blend_end = [r.f32()?, r.f32()?];
                r.i32()?;
                let group = r.i32()?;
                let events = array(b, ev_count, ev_offset, 76, &format!("sequence {name} events"))?
                    .into_iter()
                    .map(|p| {
                        let mut r = at(b, p);
                        Ok(Event {
                            frame: r.i32()?,
                            event: r.i32()?,
                            kind: r.i32()?,
                            options: fixed_name(r.take(64)?),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Sequence {
                    name,
                    fps,
                    flags,
                    activity,
                    activity_weight,
                    events,
                    frames,
                    motion_type,
                    motion_bone,
                    linear_movement,
                    mins,
                    maxs,
                    blends,
                    anim_offset,
                    blend_type,
                    blend_start,
                    blend_end,
                    group,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let groups = array(b, grp_at.0, grp_at.1, 104, "sequence groups")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                Ok(SequenceGroup {
                    label: read_name(&mut r, 32)?,
                    file: read_name(&mut r, 64)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let textures = array(b, tex_count, tex_offset, 80, "textures")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                let name = read_name(&mut r, 64)?;
                let flags = r.i32()?;
                let (w, h, px) = (r.i32()?, r.i32()?, r.i32()?);
                if w <= 0 || h <= 0 || px < 0 {
                    return err(format!("texture {name}: {w} x {h} at {px}"));
                }
                let n = (w * h) as usize;
                let data = b
                    .get(px as usize..px as usize + n + 768)
                    .ok_or_else(|| crate::Error(format!("texture {name}: pixels run past the file")))?;
                Ok(Texture {
                    name,
                    flags,
                    width: w as u32,
                    height: h as u32,
                    pixel_offset: px as u32,
                    pixels: data[..n].to_vec(),
                    palette: data[n..].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let skins = if skin_refs > 0 && skin_families > 0 {
            let cells = array(b, skin_refs * skin_families, skin_offset, 2, "skin table")?;
            cells
                .chunks(skin_refs as usize)
                .map(|row| row.iter().map(|&p| Ok(at(b, p).i16()?)).collect::<Result<Vec<_>>>())
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let body_parts = array(b, body_at.0, body_at.1, 76, "body parts")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                let name = read_name(&mut r, 64)?;
                let (count, base, offset) = (r.i32()?, r.i32()?, r.i32()?);
                let submodels = array(b, count, offset, 112, &format!("body part {name} submodels"))?
                    .into_iter()
                    .map(|p| {
                        let mut r = at(b, p);
                        let name = read_name(&mut r, 64)?;
                        r.take(8)?;
                        let (mesh_count, mesh_offset) = (r.i32()?, r.i32()?);
                        let (vc, vb, vo) = (r.i32()?, r.i32()?, r.i32()?);
                        let (nc, nb, no) = (r.i32()?, r.i32()?, r.i32()?);
                        let meshes = array(b, mesh_count, mesh_offset, 20, &format!("submodel {name} meshes"))?
                            .into_iter()
                            .map(|p| {
                                let mut r = at(b, p);
                                let (tris, cmds, skin_ref, normal_count) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
                                Ok(Mesh {
                                    triangle_count: tris,
                                    skin_ref,
                                    normal_count,
                                    commands: read_commands(b, cmds)?,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?;
                        let bytes_of = |c: i32, o: i32, what: &str| -> Result<Vec<u8>> {
                            Ok(array(b, c, o, 1, what)?.into_iter().map(|p| b[p]).collect())
                        };
                        Ok(Submodel {
                            vertex_bones: bytes_of(vc, vb, "vertex bones")?,
                            vertices: vec3s(b, vc, vo, "vertices")?,
                            normal_bones: bytes_of(nc, nb, "normal bones")?,
                            normals: vec3s(b, nc, no, "normals")?,
                            name,
                            meshes,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(BodyPart { name, base, submodels })
            })
            .collect::<Result<Vec<_>>>()?;
        let attachments = array(b, att_at.0, att_at.1, 88, "attachments")?
            .into_iter()
            .map(|p| {
                let mut r = at(b, p);
                let name = read_name(&mut r, 32)?;
                r.i32()?;
                Ok(Attachment {
                    name,
                    bone: r.i32()?,
                    position: r.f32s::<3>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Mdl {
            name: model_name,
            length,
            eye_position,
            flags,
            bones,
            controllers,
            hitboxes,
            sequences,
            groups,
            textures,
            skins,
            body_parts,
            attachments,
            bytes,
        })
    }

    /// The sequence named `name` (the first, names may repeat).
    pub fn sequence(&self, name: &str) -> Option<(usize, &Sequence)> {
        self.sequences
            .iter()
            .enumerate()
            .find(|(_, s)| s.name.eq_ignore_ascii_case(name))
    }

    /// The submodel each body part shows for an entity's `body` number.
    pub fn submodels_for_body(&self, body: i32) -> Vec<&Submodel> {
        self.body_parts
            .iter()
            .filter_map(|p| {
                let n = p.submodels.len() as i32;
                if n == 0 {
                    return None;
                }
                p.submodels.get(((body / p.base.max(1)) % n) as usize)
            })
            .collect()
    }

    /// The stored values (before scaling) of one channel of one bone in
    /// blend `blend` of sequence `seq`, one per frame. `anim` is the file
    /// holding the animation: this model's bytes for group 0, the group's
    /// sequence file otherwise. A constant channel gives all zeros.
    pub fn stored_values(
        &self,
        anim: &[u8],
        seq: usize,
        blend: usize,
        bone: usize,
        channel: usize,
    ) -> Result<Vec<i16>> {
        let s = self
            .sequences
            .get(seq)
            .ok_or_else(|| crate::Error(format!("no sequence {seq}")))?;
        let frames = s.frames.max(0) as usize;
        let record = s.anim_offset as usize + (blend * self.bones.len() + bone) * 12;
        let off = at(anim, record + channel * 2).u16()? as usize;
        if off == 0 {
            return Ok(vec![0; frames]);
        }
        let mut r = at(anim, record + off);
        let mut out = Vec::with_capacity(frames);
        while out.len() < frames {
            let (valid, total) = (r.u8()? as usize, r.u8()? as usize);
            if total == 0 {
                return err(format!(
                    "sequence {} bone {bone} channel {channel}: an empty block",
                    s.name
                ));
            }
            let mut last = 0i16;
            for i in 0..total {
                if i < valid {
                    last = r.i16()?;
                }
                if out.len() < frames {
                    out.push(last);
                }
            }
        }
        Ok(out)
    }

    /// A channel's value at a frame: default + stored × scale (no
    /// controller).
    pub fn channel_value(
        &self,
        anim: &[u8],
        seq: usize,
        blend: usize,
        bone: usize,
        channel: usize,
        frame: usize,
    ) -> Result<f32> {
        let v = self.stored_values(anim, seq, blend, bone, channel)?;
        let b = &self.bones[bone];
        Ok(b.value[channel] + *v.get(frame).unwrap_or(&0) as f32 * b.scale[channel])
    }
}

/// A sequence group file's header: `IDSQ`, version 10.
pub fn check_sequence_file(bytes: &[u8]) -> Result<()> {
    let mut r = Le::new(bytes);
    if r.take(4)? != SEQUENCE_MAGIC {
        return err("not a sequence group file (IDSQ)");
    }
    let v = r.i32()?;
    if v != VERSION {
        return err(format!("sequence group file version {v}"));
    }
    Ok(())
}
