//! Source navigation meshes (`maps/<map>.nav`, versions 9 and 16; CS:S's
//! bots read them) into the neutral `NavMesh`, per specs/cs_source/nav.md "File layout": the file is
//! read by `source_formats::nav`; here its records are linked (ids to
//! indices, ladders, encounters) and put in engine units and axes.

use bevy::prelude::*;
pub use source_formats::nav::NavInfo;
use source_formats::nav::{Area as RawArea, NavFile};

use crate::map::nav::{Encounter, HidingSpot, NavArea, NavLadder, NavMesh, Side, Via};

/// A Source-space point (units, Z up) in engine space (meters, Y up).
fn to_engine(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * super::METERS_PER_UNIT
}

/// Source compass side (N=0 is -Y) to the engine edge it becomes.
fn side(d: usize) -> Side {
    match d {
        0 => Side::MaxZ,
        1 => Side::MaxX,
        2 => Side::MinZ,
        _ => Side::MinX,
    }
}

pub fn parse(bytes: &[u8]) -> Result<(NavMesh, NavInfo), String> {
    let NavFile {
        info,
        places,
        areas: raw,
        ladders: raw_ladders,
    } = source_formats::nav::parse(bytes)?;

    let mut ladders = Vec::new();
    let mut ladder_ends: Vec<(u32, [u32; 3], u32)> = Vec::new();
    for l in &raw_ladders {
        let normal = match l.dir {
            0 => Vec3::NEG_Y,
            1 => Vec3::X,
            2 => Vec3::Y,
            _ => Vec3::NEG_X,
        };
        ladders.push(NavLadder {
            top: to_engine(l.top),
            bottom: to_engine(l.bottom),
            length: l.length * 0.0254,
            normal: to_engine(normal).normalize(),
        });
        ladder_ends.push((l.id, l.top_areas, l.bottom_area));
    }

    let index: std::collections::HashMap<u32, usize> = raw.iter().enumerate().map(|(i, a)| (a.id, i)).collect();
    let spot_at: std::collections::HashMap<u32, Vec3> = raw
        .iter()
        .flat_map(|a| a.spots.iter().map(|s| (s.0, to_engine(s.1))))
        .collect();
    // An area's visible set: its own list plus, when it names one, the
    // list of the area it inherits from (the spec doesn't say how the two
    // combine; the union is our reading).
    let visible_of = |a: &RawArea| {
        let mut list: Vec<(usize, u8)> = a
            .visible
            .iter()
            .chain(
                index
                    .get(&a.inherit)
                    .filter(|_| a.inherit != 0)
                    .map(|&i| raw[i].visible.iter())
                    .into_iter()
                    .flatten(),
            )
            .filter_map(|(id, b)| index.get(id).map(|&i| (i, *b)))
            .collect();
        list.sort_by_key(|v| (v.0, std::cmp::Reverse(v.1)));
        list.dedup_by_key(|v| v.0);
        list
    };
    let ladder_index = |id: u32| ladder_ends.iter().position(|l| l.0 == id);
    let areas = raw
        .iter()
        .map(|a| {
            let mut links = Vec::new();
            for (d, list) in a.links.iter().enumerate() {
                for target in list {
                    if *target != a.id
                        && let Some(&t) = index.get(target)
                    {
                        links.push((t, Via::Walk(side(d))));
                    }
                }
            }
            for l in a.ladders[0].iter().filter_map(|&id| ladder_index(id)) {
                for top in ladder_ends[l].1 {
                    if let Some(&t) = index.get(&top) {
                        links.push((t, Via::LadderUp(l)));
                    }
                }
            }
            for l in a.ladders[1].iter().filter_map(|&id| ladder_index(id)) {
                if let Some(&t) = index.get(&ladder_ends[l].2) {
                    links.push((t, Via::LadderDown(l)));
                }
            }
            let centre = (a.nw + a.se) / 2.0;
            NavArea {
                id: a.id,
                flags: a.flags,
                // Source y grows south; engine z = -y.
                min: Vec2::new(a.nw.x, -a.se.y) * 0.0254,
                max: Vec2::new(a.se.x, -a.nw.y) * 0.0254,
                heights: [a.sw_z, a.se.z, a.ne_z, a.nw.z].map(|h| h * 0.0254),
                center: to_engine(centre),
                links,
                place: (a.place > 0).then(|| a.place as usize - 1),
                hiding: a
                    .spots
                    .iter()
                    .map(|s| HidingSpot {
                        pos: to_engine(s.1),
                        flags: s.2,
                    })
                    .collect(),
                visible: visible_of(a),
                encounters: a
                    .encounters
                    .iter()
                    .filter_map(|(from, to, list)| {
                        Some(Encounter {
                            from: *index.get(from)?,
                            to: *index.get(to)?,
                            spots: list
                                .iter()
                                .filter_map(|(id, t)| Some((*spot_at.get(id)?, *t as f32 / 255.0)))
                                .collect(),
                        })
                    })
                    .filter(|e| !e.spots.is_empty())
                    .collect(),
            }
        })
        .collect();
    Ok((NavMesh {
        areas,
        ladders,
        places,
        generated: false,
    }, info))
}
