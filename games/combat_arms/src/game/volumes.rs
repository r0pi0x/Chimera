//! Combat Arms' volumes (specs/combat_arms/world_dat.md, "Moving world
//! models and volumes"; grenades.md section 7): the maps' volume objects
//! (`Water`, `EndlessFall`, `PoisonGas`, ...), each tied to the world
//! model of its `Name`, as `CaVolume`s in the world.
//!
//! `world` hands them over as map entities (`MapEntities`: the class, its
//! `Viscosity`, `Current`, `Damage`, `PendingSec`, `Hidden`, `Friction`,
//! `PlayerSpeedFactor`, `PlayerGravity`, `Velocity` and a jump pad's
//! `JumpDirection`, and the world model's polygons as one hull of
//! planes). `load_volumes` makes a `CaVolume` of each (class defaults
//! where a property is missing) when a map loads. A point is inside when
//! it is behind every plane; a box touches it when it overlaps the bounds
//! and isn't wholly in front of any plane (`touches_box`).
//!
//! What they do: viscosity and current slow and carry non-player objects
//! (grenades, `grenades::step`) and the liquids (`Water`,
//! `CorrosiveFluid`, `FreezingWater`) set their gravity to −200 u/s²
//! (`in_liquid`: also the effects' under-water tests and the first-person
//! muzzle flash's); players' movement in them (ladders, liquids,
//! viscosity, currents, friction, speed factors, player gravity, jump
//! pads, safety nets; by the hull's box, and the eye volume,
//! `eye_volume`) is movement.md's (`movement`); players whose hull box
//! touches one are handed `Damage` x the server step (50 ms) as lasting
//! damage each tick, applied by the frame time for 0.25 s (so `Damage` /
//! 20 per second, going on 0.25 s after leaving: `volume_damage`, on the
//! server; world_dat.md Corrections (2026-10-22) 1). Plain volumes hurt
//! players only; `DamageVolume` heals by default (`FAK_HEAL`, 5), has a
//! `TargetTeam` and `Locked` and takes bots; the kind comes from
//! `DamageType` (`typed`: `BURN` burns, `ENDLESS FALL` is the endless-fall
//! kind); `EndlessFall`'s 10⁸ kills at once, after `PendingSec` on a 0.5 s
//! grid of looks when it is set. Messages (`objects`): `DamageVal`,
//! `VELOCITY`, `LOCK`/`UNLOCK`; a launching pad plays its
//! `WorldSoundPath` (`PadLaunch`, `pad_sounds`).
//! `ca_volumes_here [radius]` lists the volumes at the player's centre
//! (`volumes_near`).
//!
//! The map's objects (`objects`) switch them (`ON`, `OFF`, `HIDDEN`: a
//! `StartupCommand`'s `msg Box_Kill01 off`, a trigger's delayed `on`) and
//! move them (a world model's `Attachments`: HGOILRIG's rising sea
//! carries its `Water` and `PoisonGas00`), each found by its map entity
//! index (`VolumeIndex`).
//!
//! A Source map's ladder brushes (`MapBrush::ladder`, any game's map) become
//! `Ladder` volumes too (`BrushLadder`, upward faces lowered 2 cm so
//! standing on top doesn't grab, side faces pushed out 2 Source units so
//! a hull stopped by the solid brush touches it), so Combat Arms' movement
//! climbs every map's ladders from any side (the spec is silent on Source
//! maps; our choice).
//!
//! A Source map's water (`MapWater`, any game's map) becomes `Water`
//! volumes too (`BrushWater`), so Combat Arms' movement swims in it. The
//! other way round, the `Water`-like liquids and `Ladder`s of Combat Arms'
//! own maps are handed to Chimera as `WaterZone`s and `LadderZone`s
//! (`sync_zones`, hidden ones left out), which any game's movement (CS:S's)
//! swims in and climbs; a ladder zone is solid to it (our choice).
//!
//! Known shortcuts: the volume is its world model's polygons' planes (the
//! exact shape for a convex brush; a concave one is taken as the
//! intersection of its faces' half-spaces); a moved volume's world model
//! is drawn where it stood (only its planes move); the
//! armour, the sound filter, fog and tint aren't read (freezing
//! damage is generic, its screen effects aren't made); `VELOCITY` reaches
//! clients with the objects' state (`objects::net`);
//! the class defaults of `Friction` (1, `Ice` 0) and `PlayerSpeedFactor`
//! (1) are ours (world_dat.md gives Ice's and Wind's only); the box test has no edge
//! axes (a box near a convex brush's edge, outside it, can count); of
//! overlapping eye volumes a liquid or ladder wins (`eye_volume`).

use std::collections::{BTreeMap, BTreeSet};

/// What an `EndlessFall` volume's damage names as its weapon: the
/// `EndlessFall` damage kind (characters.md 10).
pub const ENDLESS_FALL: &str = "combat_arms:endless_fall";

use bevy::prelude::*;

use super::{effects::sounds::CaSound, inventory, special::CaHeals};
use crate::{
    console::ConsoleAppExt,
    core::{Bot, Damage, LadderZone, WaterZone, DamageKind, Dead, Health, Hitgroup, Intent, MovementState, Npc, SimClock, SimSet, Team},
    map::{MapEntities, MapEntity, MapPart, entities::entity_to_engine},
};

/// Liquid gravity for non-player objects, u/s² (fixed in code).
pub const LIQUID_GRAVITY: f32 = -200.0;

/// The volume classes and their defaults: (class, liquid, viscosity,
/// damage per second (HP)).
pub const VOLUME_CLASSES: &[(&str, bool, f32, f32)] = &[
    ("Water", true, 0.035, 0.0),
    ("CorrosiveFluid", true, 0.035, 5.0),
    ("FreezingWater", true, 0.035, 10.0),
    ("VolumeBrush", false, 0.0, 0.0),
    ("PoisonGas", false, 0.0, 5.0),
    ("Electricity", false, 0.0, 20.0),
    ("Burn", false, 0.0, 20.0),
    ("ColdAir", false, 0.0, 5.0),
    ("EndlessFall", false, 0.0, 1.0e8),
    ("Ice", false, 0.0, 0.0),
    ("Wind", false, 0.0, 0.0),
    ("Filter", false, 0.0, 0.0),
    ("SafteyNet", false, 0.0, 0.0),
    ("Ladder", false, 0.4, 0.0),
    ("Weather", false, 0.0, 0.0),
    ("Gravity", false, 0.0, 0.0),
    ("DamageVolume", false, 0.0, 5.0),
    ("WorldVolume", false, 0.0, 0.0),
    ("JumpVolume", false, 0.0, 0.0),
];

/// A `Gravity` volume's `PlayerGravity` by default, u/s^2.
pub const DEFAULT_PLAYER_GRAVITY: f32 = -2000.0;

/// A `JumpVolume`'s `Velocity` by default, u/s (world_dat.md).
pub const DEFAULT_PAD_SPEED: f32 = 1000.0;

/// The classes the eye test never takes as the eye volume (movement.md
/// "Membership"; `CorrosiveFluid` and `Ice` by its quirk).
const NOT_EYE_VOLUMES: &[&str] = &["SafteyNet", "JumpVolume", "Weather", "CorrosiveFluid", "Ice"];

/// The classes left out of the players' ground friction (movement.md
/// "Volumes"): filters, safety nets and jump pads.
const NO_FRICTION_VOLUMES: &[&str] = &["Filter", "SafteyNet", "JumpVolume"];

/// Whether objects of `class` are volumes.
pub fn is_volume(class: &str) -> bool {
    VOLUME_CLASSES.iter().any(|(c, ..)| *c == class)
}

/// A volume in the world: engine axes, metres for its planes (outward
/// normals) and bounds; u/s for its current; HP per second.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaVolume {
    pub class: String,
    pub planes: Vec<(Vec3, f32)>,
    pub min: Vec3,
    pub max: Vec3,
    /// A liquid: −200 u/s² gravity for objects inside.
    pub liquid: bool,
    /// 0..1: speed lost, 5000 × this u/s².
    pub viscosity: f32,
    pub current: Vec3,
    /// Damage per second to players inside, HP.
    pub damage: f32,
    /// Seconds a player must be inside before it takes the damage.
    pub pending: f32,
    pub hidden: bool,
    /// `Friction`: the player's ground friction is 5 x the least of the
    /// volumes' it is in (movement.md "Constants"); 1, `Ice` 0.
    pub friction: f32,
    /// `PlayerSpeedFactor`: multiplies the player's speed limit; 1.
    pub speed_factor: f32,
    /// A `Gravity` volume's `PlayerGravity` (u/s^2 along Y, −2000): the
    /// players' gravity inside is the sum of these.
    pub player_gravity: Option<f32>,
    /// A `JumpVolume`'s launch velocity (u/s, engine axes): its
    /// `Velocity` (a speed, 1000 by default) along the forward vector of
    /// the object's `Rotation` (world_dat.md; the map entity's
    /// `JumpDirection`). A player whose hull touches the pad and stood on
    /// ground is launched by it (movement.md "Jump pads").
    pub jump_velocity: Option<Vec3>,
    /// The damage's kind (from `damage_type`): burn for `BURN`, else
    /// generic.
    pub kind: DamageKind,
    /// `DamageType`, upper case: the class's default unless the map names
    /// one (world_dat.md Corrections (2026-10-22) 4).
    pub damage_type: String,
    /// `ENDLESS FALL`: the damage is of the endless-fall kind.
    pub endless_fall: bool,
    /// `FAK_HEAL`: the damage heals (a `DamageVolume`'s default).
    pub heal: bool,
    /// A `DamageVolume`'s `Locked`: it does nothing (`LOCK` / `UNLOCK`).
    pub locked: bool,
    /// A `DamageVolume`'s `TargetTeam` (team id; None: everyone).
    pub target_team: Option<u8>,
    /// A `JumpVolume`'s world sound (`WorldSoundPath` and its radii).
    pub pad_sound: Option<PadSound>,
}

/// The sound a jump pad plays at the player it launches (world_dat.md
/// Corrections (2026-10-22) 7).
#[derive(Clone, Debug, PartialEq)]
pub struct PadSound {
    pub file: String,
    /// `SndInnerRadius`, `SndOuterRadius` (u, 300 each by default).
    pub inner: f32,
    pub outer: f32,
    /// `IgnoreTouchPlayer` (true by default): an ordinary 3D sound, else a
    /// non-3D one.
    pub three_d: bool,
}

/// The class's default `DamageType` (world_dat.md).
fn default_damage_type(class: &str) -> &'static str {
    match class {
        "VolumeBrush" => "CHOKE",
        "CorrosiveFluid" | "Burn" => "BURN",
        "FreezingWater" | "ColdAir" => "FREEZE",
        "PoisonGas" => "POISON",
        "Electricity" => "ELECTROCUTE",
        "EndlessFall" => "ENDLESS FALL",
        "DamageVolume" => "FAK_HEAL",
        _ => "UNSPECIFIED",
    }
}

impl CaVolume {
    /// A `Water` box from `min` to `max` (m) with its viscosity and
    /// current (u/s).
    pub fn liquid_box(min: Vec3, max: Vec3, viscosity: f32, current: Vec3) -> Self {
        Self {
            viscosity,
            current,
            ..Self::class_box("Water", min, max).expect("a volume class")
        }
    }

    /// A box of volume class `class` from `min` to `max` (m) with the
    /// class's defaults; None for a class that isn't a volume's.
    pub fn class_box(class: &str, min: Vec3, max: Vec3) -> Option<Self> {
        let &(class, liquid, viscosity, damage) = VOLUME_CLASSES.iter().find(|(c, ..)| *c == class)?;
        Some(Self {
            class: class.into(),
            planes: Vec::new(),
            min,
            max,
            liquid,
            viscosity,
            current: Vec3::ZERO,
            damage,
            pending: 0.0,
            hidden: false,
            friction: if class == "Ice" { 0.0 } else { 1.0 },
            speed_factor: 1.0,
            player_gravity: (class == "Gravity").then_some(DEFAULT_PLAYER_GRAVITY),
            // No rotation: forward is the file's +Z, engine -Z.
            jump_velocity: (class == "JumpVolume").then_some(Vec3::NEG_Z * DEFAULT_PAD_SPEED),
            kind: DamageKind::Generic,
            damage_type: String::new(),
            endless_fall: false,
            heal: false,
            locked: false,
            target_team: None,
            pad_sound: None,
        }
        .typed(default_damage_type(class)))
    }

    /// This volume with `DamageType` `name` (case ignored): the kind of
    /// its damage, or a heal.
    pub fn typed(mut self, name: &str) -> Self {
        let name = name.trim().to_ascii_uppercase();
        self.kind = if name == "BURN" { DamageKind::Burn } else { DamageKind::Generic };
        self.endless_fall = name == "ENDLESS FALL";
        self.heal = name == "FAK_HEAL";
        self.damage_type = name;
        self
    }

    /// A ladder (`Ladder`): characters inside climb (movement.md "Other
    /// modifiers").
    pub fn is_ladder(&self) -> bool {
        self.class == "Ladder"
    }

    /// A safety net (`SafteyNet`): no fall damage inside.
    pub fn is_safety_net(&self) -> bool {
        self.class == "SafteyNet"
    }

    /// Whether it counts for the players' ground friction (not a filter,
    /// safety net or jump pad; hidden ones do).
    pub fn sets_friction(&self) -> bool {
        !NO_FRICTION_VOLUMES.contains(&self.class.as_str())
    }

    /// Whether the axis-aligned box from `min` to `max` (m) touches it
    /// (touching counts), hidden or not: the box against its bounds and
    /// each of its planes (movement.md "Membership", the hull box).
    pub fn touches_box(&self, min: Vec3, max: Vec3) -> bool {
        let (c, h) = ((min + max) / 2.0, (max - min) / 2.0);
        min.cmple(self.max + 1e-4).all()
            && max.cmpge(self.min - 1e-4).all()
            && self.planes.iter().all(|(n, d)| n.dot(c) - n.abs().dot(h) <= d + 1e-4)
    }

    /// Whether the point `p` (m) is inside.
    pub fn contains(&self, p: Vec3) -> bool {
        !self.hidden
            && p.cmpge(self.min - 1e-4).all()
            && p.cmple(self.max + 1e-4).all()
            && self.planes.iter().all(|(n, d)| n.dot(p) <= d + 1e-4)
    }

    /// The volume of map entity `e` (`MapEntities` with `scale` metres per
    /// entity unit), if its class is a volume's and it has a hull.
    pub fn from_entity(e: &MapEntity, scale: f32) -> Option<Self> {
        let defaults = Self::class_box(e.classname(), Vec3::ZERO, Vec3::ZERO)?;
        let hull = e.hulls.first()?;
        let points: Vec<Vec3> = hull.points.iter().map(|p| entity_to_engine(*p, scale)).collect();
        let (min, max) = points
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        let real = |k: &str| e.get(k).and_then(|v| v.trim().parse::<f32>().ok());
        Some(Self {
            planes: hull
                .planes
                .iter()
                .map(|(n, d)| (entity_to_engine(*n, 1.0), d * scale))
                .collect(),
            min,
            max,
            viscosity: real("Viscosity").unwrap_or(defaults.viscosity).clamp(0.0, 1.0),
            current: e.get("Current").map_or(Vec3::ZERO, |_| e.vector("Current")),
            damage: real("Damage").unwrap_or(defaults.damage),
            pending: real("PendingSec").unwrap_or(0.0),
            hidden: e.get("Hidden").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")),
            friction: real("Friction").unwrap_or(defaults.friction),
            speed_factor: real("PlayerSpeedFactor").unwrap_or(defaults.speed_factor),
            player_gravity: defaults.player_gravity.map(|g| real("PlayerGravity").unwrap_or(g)),
            jump_velocity: defaults.jump_velocity.map(|v| {
                let dir = match e.get("JumpDirection") {
                    Some(_) => e.vector("JumpDirection"),
                    None => v.normalize_or_zero(),
                };
                dir * real("Velocity").unwrap_or(DEFAULT_PAD_SPEED)
            }),
            pad_sound: (defaults.class == "JumpVolume")
                .then(|| e.get("WorldSoundPath").map(str::trim).filter(|f| !f.is_empty()))
                .flatten()
                .map(|file| PadSound {
                    file: file.to_string(),
                    inner: real("SndInnerRadius").unwrap_or(300.0),
                    outer: real("SndOuterRadius").unwrap_or(300.0),
                    three_d: !e.get("IgnoreTouchPlayer").is_some_and(|v| v == "0" || v.eq_ignore_ascii_case("false")),
                }),
            locked: defaults.class == "DamageVolume"
                && e.get("Locked").is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")),
            target_team: e.get("TargetTeam").and_then(super::rules::objects::team_named),
            ..defaults
        }
        .typed(e.get("DamageType").unwrap_or(default_damage_type(e.classname()))))
    }
}

/// The map entity index of a volume (`MapEntities`), for the world
/// objects that switch and move it (`objects`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeIndex(pub usize);

/// Whether the point `p` (m) is inside a liquid volume (`Water`,
/// `CorrosiveFluid`, `FreezingWater`): effects_sounds.md's under-water
/// tests, view_models.md's muzzle flash.
pub fn in_liquid<'a>(volumes: impl IntoIterator<Item = &'a CaVolume>, p: Vec3) -> bool {
    volumes.into_iter().any(|v| v.liquid && v.contains(p))
}

/// The player's eye volume at the eye point `p` (m), movement.md
/// "Membership": a visible volume it is in, a `Filter` only when there is
/// no other; never a safety net, jump pad, weather, corrosive fluid or
/// ice volume. Of overlapping ones the spec takes the first found, in an
/// order we don't have: ours prefers a liquid or ladder (HGOILRIG's sea
/// lies inside a poison-gas volume), then the first.
pub fn eye_volume<'a>(volumes: impl IntoIterator<Item = &'a CaVolume>, p: Vec3) -> Option<&'a CaVolume> {
    let rank = |v: &CaVolume| match () {
        _ if v.liquid || v.is_ladder() => 0,
        _ if v.class == "Filter" => 2,
        _ => 1,
    };
    let mut found: Option<&CaVolume> = None;
    for v in volumes {
        if NOT_EYE_VOLUMES.contains(&v.class.as_str()) || !v.contains(p) {
            continue;
        }
        if found.is_none_or(|f| rank(v) < rank(f)) {
            found = Some(v);
        }
    }
    found
}

/// The map's volumes, made when it loads; players' damage inside them.
pub struct CaVolumesPlugin;

/// The volumes that hurt, drown or burn a player, as zones for spawns
/// to keep out of (`map::SpawnHazards`).
fn hazard_zones(world: &mut World) -> Vec<crate::core::MapBrush> {
    world
        .query::<&CaVolume>()
        .iter(world)
        .filter(|v| !v.hidden && (v.liquid || (v.damage > 0.0 && !v.heal && !v.locked && v.target_team.is_none())))
        .map(|v| crate::core::MapBrush {
            planes: v.planes.clone(),
            min: v.min,
            max: v.max,
            ladder: false,
            surface: None,
        })
        .collect()
}

impl Plugin for CaVolumesPlugin {
    fn build(&self, app: &mut App) {
        use crate::map::SpawnHazardAppExt;
        app.add_spawn_hazards(hazard_zones);
        app.init_resource::<VolumeDamage>().add_message::<PadLaunch>().add_systems(
            FixedUpdate,
            (
                load_volumes
                    .run_if(resource_exists_and_changed::<MapEntities>)
                    .before(super::objects::CaObjectsSet),
                load_brush_volumes.before(super::objects::CaObjectsSet),
                volume_damage
                    .run_if(crate::core::authoritative)
                    .after(super::objects::CaObjectsSet)
                    .before(super::special::apply_heals),
            )
                .chain()
                .in_set(SimSet::Weapons),
        )
        .console_command(
            "combat_arms:ca_volumes_here",
            "ca_volumes_here [radius u = 0]: the map's volumes at your centre (or within the radius of it): name, class, bounds (u, engine axes), damage, pending, hidden, whether you are inside.",
            |w, a| {
                let radius = a.first().and_then(|r| r.parse::<f32>().ok()).unwrap_or(0.0).max(0.0);
                let p = crate::console::command_player(w)?;
                let t = w.get::<Transform>(p).ok_or("no transform")?.translation;
                let centre = w.get::<MovementState>(p).map_or(t, |s| t + (s.hull_min + s.hull_max) / 2.0);
                Ok(Some(volumes_near(w, centre, radius)))
            },
        );
    }
}

/// `ca_volumes_here`'s listing: the volumes whose bounds come within
/// `radius_u` (u) of `centre` (m).
pub fn volumes_near(w: &mut World, centre: Vec3, radius_u: f32) -> String {
    let unit = super::movement::movement_data().meters_per_unit;
    let r = radius_u * unit;
    let mut lines = vec![format!("centre {:.1} (u, engine axes)", centre / unit)];
    let mut q = w.query::<(Option<&Name>, &CaVolume)>();
    for (n, v) in q.iter(w) {
        if !(centre.cmpge(v.min - r).all() && centre.cmple(v.max + r).all()) {
            continue;
        }
        lines.push(format!(
            "{}: {} {:.1}..{:.1} damage {}/s{} pending {} hidden {} friction {} viscosity {}{}",
            n.map_or("?", |n| n.as_str()),
            v.class,
            v.min / unit,
            v.max / unit,
            v.damage,
            if v.damage > 0.0 { format!(" ({:?})", v.kind) } else { String::new() },
            v.pending,
            v.hidden,
            v.friction,
            v.viscosity,
            if v.contains(centre) { " INSIDE" } else { "" }
        ));
    }
    if lines.len() == 1 {
        lines.push("no volumes".into());
    }
    lines.join("\n")
}

/// Each map entity of a volume class as a `CaVolume` (part of the map).
fn load_volumes(
    entities: Res<MapEntities>,
    old: Query<Entity, (With<CaVolume>, Without<BrushLadder>, Without<BrushWater>)>,
    mut commands: Commands,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    for (i, e) in entities.entities.iter().enumerate() {
        if let Some(v) = CaVolume::from_entity(e, entities.scale) {
            let name = match e.get("targetname") {
                Some(n) => format!("Volume {n} ({})", v.class),
                None => format!("Volume ({})", v.class),
            };
            commands.spawn((Name::new(name), v, VolumeIndex(i), MapPart));
        }
    }
}

/// Marks a `Ladder` volume made from a ladder brush of the map's
/// collision (`MapBrush::ladder`: a Source map's climbable contents), so
/// Combat Arms' movement climbs the ladders of any map, not only the
/// `Ladder` volumes of its own maps.
#[derive(Component, Debug)]
pub struct BrushLadder;

/// How far (m) a brush ladder's top face is lowered: standing on top of
/// the brush (its upward face carries the ladder flag) must not grab it,
/// as Source's movement ignores such faces (cs_source `movement.rs`).
const LADDER_TOP_INSET: f32 = 0.02;

/// How far (m) a brush ladder's side faces reach out: a Source map's
/// ladder brush is solid, so the hull stops a skin short of its face and
/// its box never touches the brush itself (head-on; at a slant only the
/// box's corners, past the round hull, did). Source's own ladder probe
/// reaches 2 units (cs_source `movement.rs`, `LADDER_REACH`); ours, the
/// spec is silent on solid ladders.
pub const LADDER_SIDE_REACH: f32 = 2.0 * 0.0254;

/// The `Ladder` volume of a ladder brush: the brush's planes, upward
/// faces lowered by `LADDER_TOP_INSET`, side faces (less than 0.7
/// vertical) pushed out by `LADDER_SIDE_REACH`.
pub fn ladder_from_brush(b: &crate::core::MapBrush) -> CaVolume {
    let reach = Vec3::new(LADDER_SIDE_REACH, 0.0, LADDER_SIDE_REACH);
    let mut v = CaVolume::class_box("Ladder", b.min - reach, b.max + reach).expect("the Ladder class");
    v.planes = b
        .planes
        .iter()
        .map(|&(n, d)| match n.y {
            y if y >= 0.7 => (n, d - LADDER_TOP_INSET),
            y if y > -0.7 => (n, d + LADDER_SIDE_REACH),
            _ => (n, d),
        })
        .collect();
    v.max.y = (v.max.y - LADDER_TOP_INSET).max(v.min.y);
    v
}

/// Marks a `Water` volume made from a water brush of the map
/// (`MapWater`: a Source map's water or slime), so Combat Arms' movement
/// swims in any map's water.
#[derive(Component, Debug)]
pub struct BrushWater;

/// The volumes made from the map's own data, in one system (an exclusive
/// system anywhere in the schedule is ambiguous with each system added):
/// replaces the brush ladders when the map's brushes change and the brush
/// waters when its water changes (or moves), and keeps the zones in step
/// (`sync_zones`).
fn load_brush_volumes(
    (brushes, water): (Option<Res<crate::core::MapBrushes>>, Option<Res<crate::core::MapWater>>),
    (old_ladders, old_waters): (Query<Entity, With<BrushLadder>>, Query<Entity, With<BrushWater>>),
    zoned: Query<
        (Entity, &CaVolume, Has<WaterZone>, Has<LadderZone>),
        (Changed<CaVolume>, Without<BrushLadder>, Without<BrushWater>),
    >,
    mut commands: Commands,
) {
    if brushes.as_ref().is_some_and(|b| b.is_changed()) {
        for e in &old_ladders {
            commands.entity(e).despawn();
        }
        for b in brushes.iter().flat_map(|b| b.0.iter()).filter(|b| b.ladder) {
            commands.spawn((Name::new("Ladder (map brush)"), ladder_from_brush(b), BrushLadder, MapPart));
        }
    }
    if water.as_ref().is_some_and(|w| w.is_changed()) {
        for e in &old_waters {
            commands.entity(e).despawn();
        }
        for w in water.iter().flat_map(|w| w.0.iter()) {
            let mut v = CaVolume::class_box("Water", w.brush.min, w.brush.max).expect("the Water class");
            v.planes = w.brush.planes.clone();
            commands.spawn((Name::new("Water (map water)"), v, BrushWater, MapPart));
        }
    }
    sync_zones(&zoned, &mut commands);
}

/// Keeps Chimera's `WaterZone` / `LadderZone` of each of the map's own
/// liquid and ladder volumes in step with it (not hidden: none), for other
/// games' movement. Volumes made from the map's brushes are those games'
/// already.
fn sync_zones(
    volumes: &Query<
        (Entity, &CaVolume, Has<WaterZone>, Has<LadderZone>),
        (Changed<CaVolume>, Without<BrushLadder>, Without<BrushWater>),
    >,
    commands: &mut Commands,
) {
    for (e, v, water, ladder) in volumes {
        let brush = |ladder: bool| {
            let mut b = if v.planes.is_empty() {
                crate::core::MapBrush::from_box(v.min, v.max)
            } else {
                crate::core::MapBrush { planes: v.planes.clone(), min: v.min, max: v.max, ladder, surface: None }
            };
            b.ladder = ladder;
            b
        };
        let mut e = commands.entity(e);
        if !v.hidden && v.liquid {
            e.insert(WaterZone(brush(false)));
        } else if water {
            e.remove::<WaterZone>();
        }
        if !v.hidden && v.is_ladder() {
            e.insert(LadderZone(brush(true)));
        } else if ladder {
            e.remove::<LadderZone>();
        }
    }
}

/// A jump pad launched a player (written by the movement, predicted or
/// not; only the server plays the sound).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct PadLaunch {
    /// Where the player is (metres).
    pub at: Vec3,
    pub sound: PadSound,
}

/// A launching pad's world sound (Corrections (2026-10-22) 7), played by
/// the server at the launched player: a 3D sound with the pad's radii, or
/// a non-3D one when `IgnoreTouchPlayer` is false.
fn pad_sound(l: &PadLaunch) -> CaSound {
    CaSound {
        file: l.sound.file.clone(),
        at: l.sound.three_d.then_some(l.at),
        radii: (l.sound.inner, l.sound.outer),
        volume: 100.0,
        source: None,
        channel: None,
        server: true,
        level: None,
    }
}

/// The volumes' damage bookkeeping (world_dat.md Corrections
/// (2026-10-22) 1 and 5). `BTreeMap`s: ordered by entity, nothing hashed.
#[derive(Resource, Default)]
struct VolumeDamage {
    /// Each volume with a `PendingSec`: when its next 0.5 s look at the
    /// players inside is due.
    next_check: BTreeMap<Entity, f64>,
    /// The players those looks listed (player, volume), with the time
    /// after which they take the damage.
    listed: BTreeMap<(Entity, Entity), f64>,
    /// The lasting (progressive) damage in force, per (player, volume).
    lasting: BTreeMap<(Entity, Entity), Lasting>,
}

/// One hand-over of lasting damage.
#[derive(Clone, Copy, Debug)]
struct Lasting {
    /// `Damage` × the step time, in health units (a heal too: positive).
    amount: f32,
    /// When it lapses.
    until: f64,
}

/// How long a hand-over of damage lasts, s.
pub const LASTING: f64 = 0.25;
/// How often a volume with a `PendingSec` looks at the players inside, s.
pub const PENDING_CHECK: f64 = 0.5;

/// Characters inside a damaging volume are handed `Damage` × the server
/// step as lasting damage each tick (replacing the last), which then
/// applies its amount × the frame time every frame for 0.25 s: so
/// `Damage` × the step per second (`Damage` / 20 at 20 Hz), going on for
/// 0.25 s after leaving. Plain volumes hurt players only; a
/// `DamageVolume` heals by default, filters by team, can be locked and
/// takes bots too.
#[allow(clippy::type_complexity)]
fn volume_damage(
    volumes: Query<(Entity, &CaVolume)>,
    characters: Query<
        (Entity, &Transform, &MovementState, &Health, Option<&Team>, Has<Bot>, Has<Npc>, Has<Dead>),
        With<Intent>,
    >,
    clock: Res<SimClock>,
    mut state: ResMut<VolumeDamage>,
    mut damage: MessageWriter<Damage>,
    mut heals: ResMut<CaHeals>,
    mut launches: MessageReader<PadLaunch>,
    mut sounds: MessageWriter<CaSound>,
) {
    for l in launches.read() {
        sounds.write(pad_sound(l));
    }
    let now = clock.now;
    let dt = clock.dt();
    let step = super::grenades::SERVER_FRAME as f32;
    // The 0.5 s look at the players inside, per volume with a pending time.
    let mut checking: Vec<Entity> = Vec::new();
    for (ve, v) in &volumes {
        if v.pending <= 0.001 || v.damage <= 0.0 {
            continue;
        }
        let next = state.next_check.entry(ve).or_insert(now);
        if now + 1e-9 >= *next {
            while *next <= now + 1e-9 {
                *next += PENDING_CHECK;
            }
            checking.push(ve);
        }
    }
    let mut inside_now: BTreeSet<(Entity, Entity)> = BTreeSet::new();
    state.lasting.retain(|_, l| now <= l.until + 1.0);
    for (p, t, ms, health, team, bot, npc, dead) in &characters {
        if health.current <= 0.0 || dead {
            continue;
        }
        let centre = t.translation + (ms.hull_min + ms.hull_max) / 2.0;
        let (lo, hi) = (t.translation + ms.hull_min, t.translation + ms.hull_max);
        let is_player = !bot && !npc;
        for (ve, v) in &volumes {
            if v.damage <= 0.0 || v.hidden || v.locked || !v.touches_box(lo, hi) {
                continue;
            }
            // Plain volumes hurt players only; a DamageVolume takes any
            // character of its team (or all of them).
            if v.class == "DamageVolume" {
                if v.target_team.is_some_and(|tt| team.map(|t| t.0) != Some(tt)) {
                    continue;
                }
            } else if !is_player {
                continue;
            }
            inside_now.insert((p, ve));
            if v.pending > 0.001 {
                // Listed by a look (due = the look's time + PendingSec),
                // damaged only once past it.
                if checking.contains(&ve) {
                    state.listed.entry((p, ve)).or_insert(now + v.pending as f64);
                }
                if !state.listed.get(&(p, ve)).is_some_and(|due| now > *due + 1e-9) {
                    continue;
                }
            }
            state.lasting.insert(
                (p, ve),
                Lasting {
                    amount: v.damage * step * inventory::HP,
                    until: now + LASTING,
                },
            );
        }
        // This character's lasting damage, the refresh above included.
        let mine: Vec<((Entity, Entity), Lasting)> =
            state.lasting.iter().filter(|((q, _), _)| *q == p).map(|(k, l)| (*k, *l)).collect();
        for ((_, ve), l) in mine {
            let Ok((_, v)) = volumes.get(ve) else {
                state.lasting.remove(&(p, ve));
                continue;
            };
            if now > l.until + 1e-9 {
                state.lasting.remove(&(p, ve));
                continue;
            }
            if v.heal {
                heals.now.push((p, l.amount * dt));
            } else {
                damage.write(Damage {
                    target: p,
                    attacker: None,
                    amount: l.amount * dt,
                    point: centre,
                    dir: Vec3::ZERO,
                    hitgroup: Hitgroup::Generic,
                    kind: v.kind,
                    // The endless fall's own damage kind (characters.md 10:
                    // its death animation `D_W`).
                    weapon: v.endless_fall.then_some(ENDLESS_FALL),
                    force: Vec3::ZERO,
                });
            }
        }
    }
    // Players missing at a look are forgotten.
    if !checking.is_empty() {
        state.listed.retain(|(p, ve), _| !checking.contains(ve) || inside_now.contains(&(*p, *ve)));
    }
}
