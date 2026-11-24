//! Breakables for network clients, which run no map logic: what breaking
//! leaves (a broken window's panes) replicates, and what breaking throws
//! (gibs, shards, falling panes, the glass impact) is sent as it happens,
//! for any game whose logic writes the map's breakable messages.
//!
//! - **Panes.** A window drawn as panes on the server (`map::BrushPanes` on
//!   a brush entity node) has a replicated `NetPanes` beside it (the
//!   node's map entity index and the panes, none once it is whole again);
//!   a client puts the same `BrushPanes` on its own copy of the node (its
//!   meshes cut and its collider made of the panes as on the server) or
//!   makes it whole (`map::breakables::make_whole`). Late joiners get the
//!   broken windows as they are.
//! - **Effects.** The server sends what its logic throws (`BreakFx`,
//!   unreliable as Source's temporary entities: a lost one is shards not
//!   drawn): brush gibs (`map::SpawnGibs`), a broken prop's pieces
//!   (`map::BreakProp`, which each client turns into pieces as the server
//!   does), shattered panes (`map::GlassShatter`), falling pane pieces
//!   (`map::FallingPane`, which shatter where each client's own piece
//!   lands) and glass impacts (`map::GlassImpact`); a client writes them as
//!   its own map's messages. What those make on the server (a prop's
//!   pieces as `SpawnGibs` with `prop`, a pane piece's `GlassShatter` with
//!   `piece`) isn't sent: each client makes its own.
//!
//! Break sounds are the logic's one-shots (`mapfx`); a breakable's removal
//! is its mover's state (`movers::NetMover`).

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{NetHooks, ordered::OrderedAppExt};
use crate::{
    core::NetRole,
    map::{
        BreakProp, BrushPanes, FallingPane, GlassImpact, GlassShatter, MapBrushEntity, SpawnGibs,
        breakables::{GibPiece, make_whole},
    },
};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SpawnGibs>()
        .add_message::<BreakProp>()
        .add_message::<GlassShatter>()
        .add_message::<FallingPane>()
        .add_message::<GlassImpact>()
        .replicate::<NetPanes>()
        .ordered::<NetPanes>()
        .add_server_message::<BreakFx>(Channel::Unreliable)
        .make_message_independent::<BreakFx>()
        .add_systems(
            PostUpdate,
            (write_panes, forward)
                .before(ServerSystems::Send)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PreUpdate,
            (copy_panes, receive)
                .after(ClientSystems::Receive)
                .run_if(resource_equals(NetRole::Client)),
        );
    app.world_mut()
        .resource_mut::<NetHooks>()
        .stopped_serving
        .push(stopped_serving);
}

/// A window's panes on the server's node (`map::BrushPanes`), node space;
/// `panes` None: whole.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetPanes {
    /// The map entity index (`map::MapBrushEntity`).
    pub index: u32,
    pub panes: Option<WirePanes>,
}

/// `map::BrushPanes` on the wire: which panes are broken as bits, row by
/// row.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WirePanes {
    pub corner: [f32; 3],
    pub u: [f32; 3],
    pub v: [f32; 3],
    pub depth: (f32, f32),
    pub cols: u16,
    pub rows: u16,
    pub broken: Vec<u8>,
}

impl WirePanes {
    pub fn of(p: &BrushPanes) -> Self {
        let mut broken = vec![0u8; p.broken.len().div_ceil(8)];
        for (i, b) in p.broken.iter().enumerate() {
            if *b {
                broken[i / 8] |= 1 << (i % 8);
            }
        }
        Self {
            corner: p.corner.to_array(),
            u: p.u.to_array(),
            v: p.v.to_array(),
            depth: p.depth,
            cols: p.cols as u16,
            rows: p.rows as u16,
            broken,
        }
    }

    pub fn panes(&self) -> BrushPanes {
        let n = self.cols as usize * self.rows as usize;
        BrushPanes {
            corner: Vec3::from_array(self.corner),
            u: Vec3::from_array(self.u),
            v: Vec3::from_array(self.v),
            depth: self.depth,
            cols: self.cols as usize,
            rows: self.rows as usize,
            broken: (0..n)
                .map(|i| self.broken.get(i / 8).is_some_and(|b| b & (1 << (i % 8)) != 0))
                .collect(),
        }
    }
}

/// Server -> clients: something breaking threw this (engine space).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BreakFx {
    /// `map::SpawnGibs` of a brush (not a prop's pieces, not a pane
    /// piece).
    Gibs {
        set: String,
        glass: bool,
        pieces: Vec<WireGib>,
        bounce: Option<String>,
    },
    /// `map::BreakProp`.
    Prop {
        prop: u32,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        velocity: [f32; 3],
        spin: [f32; 3],
        skin: Option<i32>,
    },
    /// `map::GlassShatter` (the logic's, not a pane piece's).
    Shatter {
        at: [f32; 3],
        normal: [f32; 3],
        size: [f32; 2],
        velocity: [f32; 3],
        tile: bool,
        shard: f32,
    },
    /// `map::FallingPane`.
    Fall {
        at: [f32; 3],
        axes: [[f32; 3]; 3],
        size: [f32; 2],
        body: u32,
        spin: [f32; 3],
        shard: f32,
    },
    /// `map::GlassImpact`.
    Impact { at: [f32; 3], normal: [f32; 3] },
}

/// `map::breakables::GibPiece` on the wire.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WireGib {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub spin: [f32; 3],
    pub life: f32,
    pub model: Option<u32>,
    pub rotation: Option<[f32; 4]>,
    pub frozen: bool,
    pub skin: i32,
    pub fade_dist: Option<(f32, f32)>,
    pub debris: bool,
}

impl WireGib {
    fn of(g: &GibPiece) -> Self {
        Self {
            position: g.position.to_array(),
            velocity: g.velocity.to_array(),
            spin: g.spin.to_array(),
            life: g.life,
            model: g.model.map(|m| m as u32),
            rotation: g.rotation.map(|r| r.to_array()),
            frozen: g.frozen,
            skin: g.skin,
            fade_dist: g.fade_dist,
            debris: g.debris,
        }
    }

    fn piece(&self) -> GibPiece {
        GibPiece {
            position: Vec3::from_array(self.position),
            velocity: Vec3::from_array(self.velocity),
            spin: Vec3::from_array(self.spin),
            life: self.life,
            model: self.model.map(|m| m as usize),
            rotation: self.rotation.map(Quat::from_array),
            frozen: self.frozen,
            skin: self.skin,
            fade_dist: self.fade_dist,
            debris: self.debris,
        }
    }
}

/// The server: each window node's panes into its `NetPanes` (replicated
/// from the first break on; none once whole again). Only broken windows
/// have panes, so each frame's look at them is cheap.
fn write_panes(
    windows: Query<(Entity, &MapBrushEntity, &BrushPanes, Option<&NetPanes>)>,
    whole: Query<(Entity, &NetPanes), Without<BrushPanes>>,
    mut commands: Commands,
) {
    for (node, index, panes, have) in &windows {
        let net = NetPanes {
            index: index.0 as u32,
            panes: Some(WirePanes::of(panes)),
        };
        if have != Some(&net) {
            commands.entity(node).insert((Replicated, net));
        }
    }
    for (node, have) in &whole {
        if have.panes.is_some() {
            commands.entity(node).insert(NetPanes {
                index: have.index,
                panes: None,
            });
        }
    }
}

/// Serving stopped: windows stop replicating.
fn stopped_serving(world: &mut World) {
    let windows: Vec<Entity> = world.query_filtered::<Entity, With<NetPanes>>().iter(world).collect();
    for e in windows {
        world.entity_mut(e).remove::<NetPanes>();
    }
}

/// A client: the server's panes onto its own copy of each window node
/// (or the window whole again), whenever they differ (a round restart
/// here may have made it whole before the server's word).
fn copy_panes(
    windows: Query<&NetPanes>,
    nodes: Query<(Entity, &MapBrushEntity, Option<&BrushPanes>), Without<NetPanes>>,
    mut commands: Commands,
) {
    for net in &windows {
        let Some((node, index, have)) = nodes.iter().find(|(_, n, _)| n.0 as u32 == net.index) else {
            continue;
        };
        match &net.panes {
            Some(p) => {
                let panes = p.panes();
                if have != Some(&panes) {
                    commands.entity(node).insert(panes);
                }
            }
            None if have.is_some() => {
                let index = index.0;
                commands.queue(move |w: &mut World| make_whole(w, node, index));
            }
            None => {}
        }
    }
}

/// The server: what its logic's breaking threw this frame, to every
/// client.
fn forward(
    mut gibs: MessageReader<SpawnGibs>,
    mut props: MessageReader<BreakProp>,
    mut shatters: MessageReader<GlassShatter>,
    mut falls: MessageReader<FallingPane>,
    mut impacts: MessageReader<GlassImpact>,
    mut out: MessageWriter<ToClients<BreakFx>>,
) {
    let fx = gibs
        .read()
        .filter(|g| !g.prop && g.shatters.is_none())
        .map(|g| BreakFx::Gibs {
            set: g.set.clone(),
            glass: g.glass,
            pieces: g.pieces.iter().map(WireGib::of).collect(),
            bounce: g.bounce.clone(),
        })
        .chain(props.read().map(|p| BreakFx::Prop {
            prop: p.prop as u32,
            translation: p.transform.translation.to_array(),
            rotation: p.transform.rotation.to_array(),
            scale: p.transform.scale.to_array(),
            velocity: p.velocity.to_array(),
            spin: p.spin.to_array(),
            skin: p.skin,
        }))
        .chain(shatters.read().filter(|s| !s.piece).map(|s| BreakFx::Shatter {
            at: s.at.to_array(),
            normal: s.normal.to_array(),
            size: s.size.to_array(),
            velocity: s.velocity.to_array(),
            tile: s.tile,
            shard: s.shard,
        }))
        .chain(falls.read().map(|f| BreakFx::Fall {
            at: f.at.to_array(),
            axes: f.axes.map(|a| a.to_array()),
            size: f.size.to_array(),
            body: f.body as u32,
            spin: f.spin.to_array(),
            shard: f.shard,
        }))
        .chain(impacts.read().map(|i| BreakFx::Impact {
            at: i.at.to_array(),
            normal: i.normal.to_array(),
        }));
    for message in fx {
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message,
        });
    }
}

/// A client: the server's break effects as its own map's messages.
fn receive(mut fx: MessageReader<BreakFx>, mut commands: Commands) {
    for f in fx.read().cloned() {
        commands.queue(move |w: &mut World| match f {
            BreakFx::Gibs {
                set,
                glass,
                pieces,
                bounce,
            } => {
                w.write_message(SpawnGibs {
                    set,
                    glass,
                    prop: false,
                    pieces: pieces.iter().map(WireGib::piece).collect(),
                    shatters: None,
                    bounce,
                });
            }
            BreakFx::Prop {
                prop,
                translation,
                rotation,
                scale,
                velocity,
                spin,
                skin,
            } => {
                w.write_message(BreakProp {
                    prop: prop as usize,
                    transform: Transform {
                        translation: Vec3::from_array(translation),
                        rotation: Quat::from_array(rotation),
                        scale: Vec3::from_array(scale),
                    },
                    velocity: Vec3::from_array(velocity),
                    spin: Vec3::from_array(spin),
                    skin,
                });
            }
            BreakFx::Shatter {
                at,
                normal,
                size,
                velocity,
                tile,
                shard,
            } => {
                w.write_message(GlassShatter {
                    at: Vec3::from_array(at),
                    normal: Vec3::from_array(normal),
                    size: Vec2::from_array(size),
                    velocity: Vec3::from_array(velocity),
                    tile,
                    shard,
                    piece: false,
                });
            }
            BreakFx::Fall {
                at,
                axes,
                size,
                body,
                spin,
                shard,
            } => {
                w.write_message(FallingPane {
                    at: Vec3::from_array(at),
                    axes: axes.map(Vec3::from_array),
                    size: Vec2::from_array(size),
                    body: body as usize,
                    spin: Vec3::from_array(spin),
                    shard,
                });
            }
            BreakFx::Impact { at, normal } => {
                w.write_message(GlassImpact {
                    at: Vec3::from_array(at),
                    normal: Vec3::from_array(normal),
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_survive_the_wire() {
        let p = BrushPanes {
            corner: Vec3::new(1.0, 2.0, 3.0),
            u: Vec3::X * 0.3,
            v: Vec3::Y * 0.3,
            depth: (-0.01, 0.02),
            cols: 10,
            rows: 7,
            broken: (0..70).map(|i| i % 3 == 0 || i == 69).collect(),
        };
        let w = WirePanes::of(&p);
        assert_eq!(w.broken.len(), 9);
        assert_eq!(w.panes(), p);
    }
}
