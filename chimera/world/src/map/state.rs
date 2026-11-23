//! State the logic layer (and network replication) writes on spawned map
//! parts and the map and its presentation apply: entity parts on/off,
//! prop looks and sequences, light styles, soundscape switches, tonemap
//! inputs, view kicks; what culling reads of the logic (`VisClusters`,
//! `LogicHidden`, areaportal and occluder states); render looks and brush
//! tints; the monitor camera; the effects the logic fires (sparks, tesla
//! arcs, strikes, map muzzle flashes) and decals to place; and the
//! particle rules' random numbers, which the weapon layer shares.

use bevy::prelude::*;

use super::{DEFAULT_AUTO_EXPOSURE, MapAlpha, MapHdr};

/// A kick to a player's view from the map (env_viewpunch): pitch up, yaw
/// left and roll (as `MovementState::view_roll`), radians, added to its
/// view punch (the weapon layer's `ViewPunch`, which decays it as recoil).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ViewKick {
    pub player: Entity,
    pub angles: Vec3,
}

/// A drawn part of a map entity the logic switches (sprites, dust
/// volumes): `on` shows a sprite or lets dust spawn motes; `exists` false
/// (killed) removes it from view at once. The logic layer writes it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityPart {
    /// Index into `MapData::entities`.
    pub entity: usize,
    pub on: bool,
    pub exists: bool,
}

/// An env_steam's value inputs (JetLength, SpreadSpeed, Speed), in
/// meters and m/s; None keeps the map's value. The logic layer writes it
/// on the jet's `EntityPart`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SteamInputs {
    pub length: Option<f32>,
    pub spread: Option<f32>,
    pub speed: Option<f32>,
}

/// How a prop entity looks, as the logic says (skin family, and the body
/// number SetBodyGroup set; `MapModel::skins`, `MapModel::body_choice`).
/// The logic layer writes it; the map applies it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PropLook {
    pub skin: i32,
    pub body_group: Option<i32>,
}

/// What an animated prop plays, as the logic says: a sequence by name
/// with a serial (a new serial restarts it), the one played at spawn and
/// after a sequence ends, and the round (a new round starts over).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct PropSequence {
    pub play: Option<(String, u32)>,
    pub default: String,
    pub round: u32,
}

/// Switchable light styles that are off (Source styles 32+). The logic
/// layers write it, each its own list (`with`): in an app running every
/// game's logic (the gun game) Source's logic and a Combat Arms map's
/// light groups both write it, and when each replaced the whole list they
/// undid each other several times a second, every write a whole lightmap
/// rebuild (feedback b37d546e, roadkill). The map relights the lightmaps
/// when it changes.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct LightStyles {
    /// Style numbers and whether each is lit, every writer's together (a
    /// later writer's entry for a style an earlier one lists wins);
    /// unlisted styles keep their state at map start.
    pub styles: Vec<(u8, bool)>,
    /// Each writer's own list, by name, in the order they first wrote.
    writers: Vec<(&'static str, Vec<(u8, bool)>)>,
}

impl LightStyles {
    /// One writer's styles alone.
    pub fn of(writer: &'static str, styles: Vec<(u8, bool)>) -> Self {
        Self::default().with(writer, styles).unwrap_or_default()
    }

    /// These styles with `writer`'s list now `styles` (an empty list
    /// drops the writer); None when that changes nothing, so a writer
    /// inserts the resource only when its own list changed.
    pub fn with(&self, writer: &'static str, styles: Vec<(u8, bool)>) -> Option<Self> {
        let at = self.writers.iter().position(|(w, _)| *w == writer);
        let mut next = self.clone();
        match (at, styles.is_empty()) {
            (Some(i), _) if next.writers[i].1 == styles => return None,
            (None, true) => return None,
            (Some(i), true) => {
                next.writers.remove(i);
            }
            (Some(i), false) => next.writers[i].1 = styles,
            (None, false) => next.writers.push((writer, styles)),
        }
        next.styles.clear();
        for (_, list) in &next.writers {
            for &(n, on) in list {
                match next.styles.iter_mut().find(|s| s.0 == n) {
                    Some(s) => s.1 = on,
                    None => next.styles.push((n, on)),
                }
            }
        }
        Some(next)
    }
}

/// Which soundscape volumes (by entity index, `SoundscapeZone::entity`)
/// the listener touches, as the logic layer's touch code says; None: the
/// map tests the zones' boxes itself.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct SoundscapeTouches(pub Option<Vec<usize>>);

/// Which env_soundscape entities (by entity index,
/// `SoundscapeEmitter::entity`) are enabled, as the logic layer's
/// Enable/Disable inputs left them; None: the map goes by their
/// StartDisabled keys.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct SoundscapeSwitches(pub Option<Vec<usize>>);

/// What the map's env_tonemap_controller inputs set while it runs (the
/// logic layer writes it): each set field replaces the map's look
/// (`MapHdr`), which holds what the map sets at its start.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TonemapInputs {
    pub exposure_min: Option<f32>,
    pub exposure_max: Option<f32>,
    pub bloom_scale: Option<f32>,
    pub rate: Option<f32>,
    /// UseDefaultAutoExposure: bounds back at `DEFAULT_AUTO_EXPOSURE`
    /// (until set again).
    pub default_exposure: bool,
}

impl TonemapInputs {
    /// `hdr` with these inputs applied.
    pub fn apply(&self, hdr: &MapHdr) -> MapHdr {
        let mut out = hdr.clone();
        if let Some((lo, hi)) = out.exposure {
            let (lo, hi) = if self.default_exposure {
                DEFAULT_AUTO_EXPOSURE
            } else {
                (lo, hi)
            };
            out.exposure = Some((self.exposure_min.unwrap_or(lo), self.exposure_max.unwrap_or(hi)));
        }
        if let Some(b) = self.bloom_scale {
            out.bloom_scale = b;
        }
        if let Some(r) = self.rate {
            out.rate = r;
        }
        out
    }
}

/// Areaportals map logic closed (by key); every other portal is open.
/// Written by the logic (linked doors, Open/Close inputs), read by `cull`.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct AreaPortalStates {
    /// Sorted.
    pub closed: Vec<u16>,
}

impl AreaPortalStates {
    pub fn is_open(&self, key: u16) -> bool {
        self.closed.binary_search(&key).is_err()
    }
}

/// Occluders map logic turned off (StartActive 0, Deactivate, Toggle):
/// their keys, sorted; every other occluder is active. Without it each
/// occluder's `start_active` holds.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct OccluderStates {
    pub inactive: Vec<u16>,
}

/// Clusters a map part touches; it is drawn only while one of them is
/// potentially visible from the camera's cluster. Entities without it are
/// always drawn. The culling system owns their `Visibility` (except glow
/// sprites and water surfaces, which read `potentially_visible`).
#[derive(Component, Clone, Debug)]
pub struct VisClusters {
    pub clusters: Box<[u32]>,
    pub potentially_visible: bool,
}

impl VisClusters {
    pub fn new(clusters: Vec<u32>) -> Self {
        Self {
            clusters: clusters.into_boxed_slice(),
            potentially_visible: true,
        }
    }
}

/// A map part game logic removed or turned off (a broken or killed prop):
/// culling keeps it hidden. Whoever adds it hides the part; whoever
/// removes it shows it again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LogicHidden;

/// Source render modes that matter here.
pub const RENDER_NORMAL: u8 = 0;
pub const RENDER_TRANS_COLOR: u8 = 1;
pub const RENDER_TRANS_ALPHA: u8 = 4;
pub const RENDER_TRANS_ADD: u8 = 5;
/// Where a brush entity in render mode 4 cuts its texture's alpha out
/// (Source's `$alphatestreference` default).
pub const ALPHA_TEST_REFERENCE: f32 = 0.7;
pub const RENDER_NONE: u8 = 10;

/// How an entity is drawn: colour (sRGB 0-255), alpha (renderamt) and
/// render mode.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenderLook {
    pub color: [u8; 3],
    pub alpha: u8,
    pub mode: u8,
}

impl Default for RenderLook {
    fn default() -> Self {
        Self {
            color: [255; 3],
            alpha: 255,
            mode: RENDER_NORMAL,
        }
    }
}

impl RenderLook {
    /// Drawn as its materials are.
    pub fn is_plain(&self) -> bool {
        self.color == [255; 3] && self.blend().is_none()
    }

    /// The blend its render mode asks for, with the opacity (0-1): normal
    /// mode ignores renderamt; colour, texture and alpha modes (1, 2, 4)
    /// blend by it; glow and additive modes (3, 5, 7, 8, 9) add, scaled by
    /// it; environmental (6) and none (10) aren't drawn.
    pub fn blend(&self) -> Option<(MapAlpha, f32)> {
        let a = self.alpha as f32 / 255.0;
        match self.mode {
            RENDER_NORMAL => None,
            1 | 2 | 4 => (self.alpha < 255).then_some((MapAlpha::Blend, a)),
            3 | 5 | 7 | 8 | 9 => Some((MapAlpha::Add, a)),
            6 | RENDER_NONE => Some((MapAlpha::Blend, 0.0)),
            _ => None,
        }
    }

    /// The colour multiplier (linear rgb; the game multiplies in gamma
    /// space, so the colour is converted as sRGB), with the opacity in w;
    /// an added look takes the opacity into its colour.
    pub fn factor(&self) -> Vec4 {
        self.factor_of(self.blend())
    }

    /// Render mode 1 (kRenderTransColor) on a brush entity: its surfaces
    /// draw the flat `rendercolor`, not their texture.
    pub fn flat_color(&self) -> bool {
        self.mode == RENDER_TRANS_COLOR
    }

    /// What a brush entity's surfaces blend as: like `blend`, except mode 4
    /// (kRenderTransAlpha) alpha-tests at the texture's alpha
    /// (`ALPHA_TEST_REFERENCE`), whatever renderamt says, instead of
    /// blending.
    pub fn surface_blend(&self) -> Option<(MapAlpha, f32)> {
        if self.mode == RENDER_TRANS_ALPHA && self.alpha > 0 {
            Some((MapAlpha::Mask(ALPHA_TEST_REFERENCE), 1.0))
        } else {
            self.blend()
        }
    }

    /// A brush entity's surfaces look different from their material's.
    pub fn changes_surface(&self) -> bool {
        !self.is_plain() || self.flat_color() || self.mode == RENDER_TRANS_ALPHA
    }

    /// `factor` for a brush entity's surfaces (`surface_blend`).
    pub fn surface_factor(&self) -> Vec4 {
        self.factor_of(self.surface_blend())
    }

    fn factor_of(&self, blend: Option<(MapAlpha, f32)>) -> Vec4 {
        let c = Color::srgb_u8(self.color[0], self.color[1], self.color[2]).to_linear();
        let rgb = Vec3::new(c.red, c.green, c.blue);
        match blend {
            Some((MapAlpha::Add, a)) => (rgb * a).extend(1.0),
            Some((_, a)) => rgb.extend(a),
            None => rgb.extend(1.0),
        }
    }

    /// Not drawn at all.
    pub fn invisible(&self) -> bool {
        matches!(self.blend(), Some((MapAlpha::Blend, a)) if a <= 0.0)
    }
}

/// A brush entity node's look (white, normal: as the material is) and
/// the texture frame its animated textures show (env_texturetoggle; 0:
/// the first).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrushTint(pub RenderLook, pub u32);

impl BrushTint {
    pub fn is_white(&self) -> bool {
        !self.0.changes_surface() && self.1 == 0
    }
}

/// Where the active monitor camera is (engine space, looking down -Z) and
/// its vertical field of view (degrees); None: no monitor shows anything.
/// With it, its fog (None: none).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct MonitorCamera(pub Option<(Vec3, Quat, f32)>, pub Option<MonitorFog>);

/// Every view a camera drawing into an image may show on this map, for
/// any game: Source's point_cameras that a func_monitor names, on or off
/// (one turned on later shows too); a portal's view when a game has them.
/// A map settling behind its loading screen draws each once
/// (`render::monitor`'s `warm_target_views`, counted by the settle through
/// `map::loading::LoadPending`), so what it shows is loaded, prepared and
/// its pipelines compiled before the first view opens
/// (`client::first_view`), not when the camera first turns on. The
/// game's layer fills it as the map spawns; the unload empties it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TargetViews(pub Vec<TargetView>);

/// One of `TargetViews`: where it is seen from (engine space, looking
/// down -Z), its vertical field of view (degrees) and its own fog (None:
/// none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetView {
    pub at: Vec3,
    pub rotation: Quat,
    pub fov: f32,
    pub fog: Option<MonitorFog>,
}

/// A point_camera's own fog (public entity docs): on or off, colour (sRGB
/// 0-1), start and end (meters here; the logic gives entity units) and max
/// density.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MonitorFog {
    pub enabled: bool,
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
    pub max_density: f32,
}

/// A burst of sparks (env_spark; visual_entities.md 8): where (engine
/// space), its direction (zero: none) and magnitude; a game draws it.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct SparkBurst {
    pub at: Vec3,
    pub dir: Vec3,
    pub magnitude: f32,
}

/// One spark of a point_tesla (the logic times them).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeslaSpark {
    /// The entity (index into `MapData::entities`).
    pub entity: usize,
}

/// Marks a drawn part whose transform the logic sets to its map entity's
/// pose each tick (it follows a parent).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FollowsEntity;

/// An env_muzzleflash fired (by map index).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MuzzleFlashFire {
    pub entity: usize,
}

/// A strike: the beam from `start` to `end` (engine space) lives `life`
/// seconds (0: until the map's next round restart).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct BeamStrike {
    pub entity: usize,
    pub start: Vec3,
    pub end: Vec3,
    pub life: f32,
}

/// Put a decal from `group` on the world at `point`, on the surface with
/// `normal`; `dir` is the direction of whatever made it. `spin` turns it
/// randomly in the surface plane (bullet holes); otherwise walls keep it
/// upright. With a `target` prop (or one of its colliders) the decal goes
/// on that prop's model and moves with it; otherwise on the world.
#[derive(Message, Clone, Debug)]
pub struct PlaceDecal {
    pub target: Option<Entity>,
    pub group: DecalGroup,
    pub point: Vec3,
    pub normal: Vec3,
    pub dir: Vec3,
    pub spin: bool,
    /// Goes at the next round restart (the bomb's scorch, as in CS:S);
    /// otherwise it stays until the map changes (`decal::DecalSettings`).
    pub round: bool,
}

/// Put a decal from `group` on `target`'s drawn body (a model decal,
/// following its skin; `render::body_decals`): where the shot from `from`
/// along `dir` met it at `point` (on its hitbox). `owner` made it (none on
/// himself, nor on a teammate without friendly fire). Each game's impacts
/// write it for the hits whose game decals bodies (specs/tf2/effects.md
/// 8b; CS:S's and HL2:DM's impact on a player).
#[derive(Message, Clone, Debug)]
pub struct PlaceBodyDecal {
    pub target: Entity,
    pub owner: Entity,
    pub group: DecalGroup,
    pub from: Vec3,
    pub point: Vec3,
    pub dir: Vec3,
}

/// Remove every decal on this character's body (TF2: healed to full, a
/// disguise changed; specs/tf2/effects.md 8b).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearBodyDecals(pub Entity);

/// On a character: its body takes no decals while it is there, and its
/// arrival removes those it has (TF2's cloak, disguise and Über,
/// specs/tf2/effects.md 8b).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct NoBodyDecals;

/// On a character: its ragdoll doesn't keep the living body's decals
/// (TF2's ragdoll is a model of its own, specs/tf2/effects.md 8b). Without
/// it the body, decals and all, becomes the ragdoll (CS:S, HL2:DM:
/// specs/cs_source/ragdolls.md 2.4).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct BodyDecalsOffRagdoll;

/// Which decals to choose from.
#[derive(Clone, Debug)]
pub enum DecalGroup {
    /// A named group (e.g. the knife's slashes).
    Named(String),
    /// What a surface with this game material letter takes when shot.
    Material(char),
    /// What a surface with this game material letter takes when this
    /// game's gun shoots it: the game's own group where the map holds one
    /// (`MapDecals::material_group`), else `Material`'s.
    GameMaterial(String, char),
}

/// Most decals a server keeps for late joiners; the oldest goes first.
/// (Each client draws at most `DecalSettings::limit`.)
pub const MAX_DECALS: usize = 512;

/// Random numbers for the particle rules (xorshift).
#[derive(Clone, Debug)]
pub struct ParticleRng(u64);

impl ParticleRng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// Uniform in 0..1.
    pub fn random(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform float in a..b.
    pub fn float(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.random()
    }

    /// Uniform integer in a..=b.
    pub fn int(&mut self, a: i32, b: i32) -> i32 {
        if b <= a {
            return a;
        }
        (a + (self.random() * (b - a + 1) as f32) as i32).min(b)
    }

    /// Three independent draws in a..b.
    pub fn vec3(&mut self, a: f32, b: f32) -> Vec3 {
        Vec3::new(self.float(a, b), self.float(a, b), self.float(a, b))
    }
}

/// A one-shot sound map logic played here (a locked door, a break, a
/// spark); a network server sends it to every client, which runs no map
/// logic (`net::mapfx`). The logic writes it (`logic::bridge`).
#[derive(Message, Clone, Debug)]
pub struct LogicOneShot(pub super::PlaySound);

/// A map's `say` line (point_servercommand): chat from the server, as
/// Source's server console says it: on a listen server under the host's
/// name (its own chat line), on a dedicated server as "Console"
/// (`net::chat`); in a game of one, the local player's line
/// (`client::chat`). The logic writes it (`logic::hud`).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct MapSay(pub String);

#[cfg(test)]
mod tests {
    use super::LightStyles;

    /// Two writers keep their own lists: one writing again what it wrote
    /// changes nothing, whatever the other wrote since (the gun game's
    /// Source logic and Combat Arms' light groups undid each other).
    #[test]
    fn light_style_writers_keep_their_own_lists() {
        let a = LightStyles::default().with("source", vec![]);
        assert!(a.is_none(), "an empty list on nothing changes nothing");
        let b = LightStyles::default().with("ca", vec![(32, false)]).unwrap();
        assert_eq!(b.styles, [(32, false)]);
        assert!(b.with("source", vec![]).is_none(), "Source's empty list leaves Combat Arms' alone");
        assert!(b.with("ca", vec![(32, false)]).is_none());
        let c = b.with("source", vec![(33, true), (32, true)]).unwrap();
        assert_eq!(c.styles, [(32, true), (33, true)], "the later writer wins a shared style");
        let d = c.with("source", vec![]).unwrap();
        assert_eq!(d.styles, [(32, false)], "a writer's emptied list drops its styles");
        assert_eq!(d.with("ca", vec![]).unwrap().styles, []);
    }
}
