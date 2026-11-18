use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{character::CharacterPartsExt, later::Later};
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewBackPush, ViewPunch,
        WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp,
        random::{Rng, shared_random},
    },
};

use crate::game::characters;
use crate::game::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaAltVolley, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaFocusBlur, CaZoom, CaZoomSound, Scope, Sway, ZOOM, ZoomParams},
};

use super::*;

// ---------------------------------------------------------------------------
// Plugin

/// Combat Arms' weapons: its hooks, the systems around the weapon frame,
/// and the weapons registered from its data (`data::weapon_set`).
pub struct CaWeaponsPlugin;

impl Plugin for CaWeaponsPlugin {
    fn build(&self, app: &mut App) {
        power::plugin(app);
        harm::plugin(app);
        // Its guns' short names (data/display_names.toml: the split on, and
        // overrides).
        chimera_sim::weapon::names::DisplayNamesAppExt::add_display_names(
            app,
            "combat_arms data/display_names.toml",
            include_str!("../../../data/display_names.toml"),
        );
        // Its map pool's rules and measurements (beside the draw's
        // exclusions: what an automatic gun game takes of it).
        crate::game::parts::add_map_pool(app);
        app.register_weapon_hook::<DamageParams>(DAMAGE, |e, d: &'static WeaponData, p| {
            let mut dmg = CaDamage::new(p, d.units.length, d.units.damage);
            if p.reload_third_person.is_none() {
                dmg.reload_third_person = e.get::<crate::weapon::Magazine>().map_or(0.0, |m| m.reload_time);
            }
            e.insert(dmg);
        })
        .register_weapon_hook::<SpreadParams>(SPREAD, |e, d, p| {
            e.insert(CaSpread::new(p.clone(), d.units.length));
        })
        .register_weapon_hook::<RecoilParams>(RECOIL, |e, _, p| {
            e.insert(CaRecoil::new(*p));
        })
        .register_weapon_hook::<CaHandling>(HANDLING, |e, _, p| {
            e.insert(*p);
        })
        .register_weapon_hook::<ZoomParams>(ZOOM, |e, _, p| {
            e.insert((CaZoom::new(p.clone()), crate::weapon::OwnZoom));
        })
        .register_weapon_hook::<SpinParams>(SPIN, |e, _, p| {
            e.insert(CaSpin {
                params: *p,
                state: SpinState::Idle,
            });
        })
        .register_weapon_hook::<CaAltFire>(ALT_FIRE, |e, _, p| {
            e.insert((*p, p.burst(), classes::CaAltVolley::default()));
        })
        .register_weapon_hook::<timing::TimingParams>(timing::TIMING, timing::apply)
        // An infected's weapon: a mark for the gun game's rungs only.
        .register_weapon_hook::<toml::Table>(crate::game::rungs::INFECTED, |_, _, _| {})
        // The server's timing goes to its clients (`weapon::params`).
        .weapon_param(timing::TIMING, timing::encode, timing::push);
        // The fire-mode key (`+firemode`): steps a gun's fire modes
        // (`AltModes::key`, the merge's `fire_modes_patch`).
        crate::core::ActionAppExt::intent_action(app, crate::game::data::FIRE_MODE_ACTION);
        let data = crate::game::data::CaData::of(app);
        if let Err(e) = app.add_weapon_set(data.weapon_set()) {
            panic!("Combat Arms weapon data: {e}");
        }
        // A network client whose documents differ takes the server's.
        app.server_weapon_docs(crate::game::data::served::WEAPONS, data.weapon_set(), data.weapon_docs().to_vec());
        if !app.world().contains_resource::<CaServerSettings>() {
            app.insert_resource(CaServerSettings::of(&data));
        }
        app.init_resource::<SpreadScales>()
            .init_resource::<timing::ViewTimes>()
            .init_resource::<SpreadOptions>()
            .init_resource::<CaAiDamage>()
            .insert_resource(CaSurfaces::built_in().clone())
            .add_message::<WeaponEvent>()
            .add_message::<zoom::CaZoomSound>()
            .init_resource::<zoom::CaFocusBlur>()
            .add_systems(Update, zoom::focus_blur)
            .add_message::<Damage>()
            .init_resource::<PelletCarryOver>()
            .add_systems(
                Predict::Weapons,
                (
                    // The class rules and the zoom step after the hands
                    // (`inventory`), then the cone the frame shows.
                    (classes::class_rules, zoom::zoom_step, before_shots)
                        .chain()
                        .after(crate::game::inventory::hands)
                        .before(WeaponFrame),
                    (after_shots, zoom::bolt_zoom_out)
                        .chain()
                        .after(WeaponFrame)
                        .before(crate::weapon::impact::ApplyImpulses),
                ),
            )
            // Zoom presses before movement (the scoped speed limit from
            // the press's tick), after selection (a switch resets).
            .add_systems(
                Predict::Select,
                zoom::zoom_input.after(crate::weapon::SelectWeapons).after(apply_handling),
            )
            .add_systems(FixedUpdate, armour_follows_identity.run_if(crate::core::authoritative))
            // In the predicted tick: a client replaying a switch moves
            // with the penalty of the weapon it held then.
            .add_systems(Predict::Select, apply_handling.after(crate::weapon::SelectWeapons))
            .on_every_character::<HandlingOf>()
            .on_every_character::<crate::game::movement::HeldMovePenalty>()
            // The meters are predicted with the weapon and go to the
            // client predicting it (`weapon::sync`); the kick with the
            // player.
            .predicted_part::<CaSpread>()
            .predicted_part::<CaZoom>()
            .predicted_part::<CaSpin>()
            .predicted_net::<CaKick>();
    }
}

impl NetPart for CaSpread {
    /// The meters, the last shot, the sub-step clock, the last body yaw
    /// and its twist, the jump flags (the parameters are the data's).
    type Net = (f32, f32, Option<f64>, f64, Option<Vec2>, Option<f32>, bool, bool);
    fn net(&self) -> Self::Net {
        (
            self.fire,
            self.movement,
            self.last_shot,
            self.clock,
            self.prev_view,
            self.body_yaw,
            self.jumped,
            self.was_on_ground,
        )
    }
    fn set_net(&mut self, (fire, movement, last_shot, clock, prev_view, body_yaw, jumped, was_on_ground): Self::Net) {
        self.fire = fire;
        self.movement = movement;
        self.last_shot = last_shot;
        self.clock = clock;
        self.prev_view = prev_view;
        self.body_yaw = body_yaw;
        self.jumped = jumped;
        self.was_on_ground = was_on_ground;
    }
}

/// Combat Arms' armour on its own characters only (`slots::
/// GameIdentity::allows`; where identity follows the weapon, once it has
/// one): one that first has health gets the default
/// armour; one that becomes Combat Arms' (another game's armour, or none)
/// puts on the default gear over the armour it has (its amount kept);
/// one that becomes another game's takes the gear off (what covers it is
/// then its armour's default kit: CS:S's kevlar), but never pieces another
/// game marked its own (`weapon::PiecesOf`: Halo's shield). docs/DESIGN.md
/// "Hits across games".
#[allow(clippy::type_complexity)]
pub(super) fn armour_follows_identity(
    characters: Query<
        (
            Entity,
            Has<Armor>,
            Has<ArmorPieces>,
            Option<&crate::slots::GameIdentity>,
            Has<crate::slots::FollowsWeapon>,
            Option<&chimera_sim::weapon::PiecesOf>,
        ),
        (With<Health>, With<Intent>),
    >,
    minigame: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    mut commands: Commands,
) {
    // A minigame whose players' identity follows their weapon: a player
    // without one yet (before it is taken in, or holding nothing) isn't
    // Combat Arms'.
    let following = minigame.is_some_and(|m| m.def.slots.identity == Some(chimera_sim::minigame::def::IdentityDef::Weapon));
    for (e, armor, pieces, identity, follows, owner) in &characters {
        if (follows || following) && identity.is_none() {
            continue;
        }
        let ours = crate::slots::GameIdentity::allows(identity, crate::game::GAME);
        // Another game's pieces (`weapon::PiecesOf`: Halo's shield) are
        // its own to take off.
        let pieces = pieces && owner.is_none_or(|o| o.0 == crate::game::GAME);
        match (ours, armor, pieces) {
            (true, false, _) => {
                commands.entity(e).insert(default_armour());
            }
            (true, true, false) => {
                commands.entity(e).insert(pieces_of(&[DEFAULT_BODY_GEAR]));
            }
            (false, _, true) => {
                commands.entity(e).remove::<ArmorPieces>();
            }
            _ => {}
        }
    }
}

/// An angle in (-pi, pi].
pub(super) fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let a = a.rem_euclid(tau);
    if a > std::f32::consts::PI { a - tau } else { a }
}

/// Before the frame: each active weapon's cone is what the HUD reads
/// (`Hitscan::spread`, a disc of n / 1000). The recoil returns after the
/// frame (`after_shots`): the shot leaves along the view as it was before
/// this tick's return (Corrections 11).
#[allow(clippy::type_complexity)]
pub(super) fn before_shots(
    owners: Query<&Inventory>,
    mut weapons: Query<(
        Option<&CaSpread>,
        Option<&mut Hitscan>,
        Option<&AltModes>,
        Option<&Burst>,
        Option<&Zoom>,
        Option<&CaZoom>,
    )>,
) {
    for inv in &owners {
        let Some(w) = inv.active else { continue };
        let Ok((Some(s), Some(mut scan), modes, burst, zoom, ca_zoom)) = weapons.get_mut(w) else {
            continue;
        };
        let mode = mode_of(modes.map_or(0, |m| m.current), burst, zoom, ca_zoom);
        let n = s.cone(mode) / 1000.0;
        let want = SpreadShape::Disc {
            inaccuracy: n,
            spread: 0.0,
        };
        if scan.spread != want {
            scan.spread = want;
        }
    }
}

/// One round to deliver (a `Fired` event of a Combat Arms weapon).
#[derive(Clone, Copy, Debug)]
pub(super) struct Round {
    owner: Entity,
    weapon: Entity,
    origin: Vec3,
    yaw: f32,
    pitch: f32,
    seed: u32,
    mode: u8,
    replay: bool,
}

/// A character as the ray sees it.
pub(super) struct Body {
    entity: Entity,
    /// Its origin (the near-miss test's plane side).
    origin: Vec3,
    /// World box (min, max).
    min: Vec3,
    max: Vec3,
    /// Its body's forward and right (the legs': Corrections 6).
    forward: Vec3,
    right: Vec3,
    /// Its hit shapes in the world (`body_shapes`), made when a ray first
    /// reaches its box (every test of the shapes comes after a box test):
    /// a body no round comes near is never posed (`characters::CaPose`).
    shapes: Later<crate::core::Hitboxes>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Delivery<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    characters: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Intent,
            &'static MovementState,
            &'static Health,
            Option<&'static ColliderAabb>,
            Option<&'static HitSpheres>,
            (
                Option<&'static characters::CaPose>,
                Option<&'static crate::core::Hitboxes>,
                Option<&'static crate::slots::GameIdentity>,
            ),
        ),
    >,
    armor: Query<'w, 's, (&'static mut Armor, Option<&'static ArmorPieces>)>,
    infected: Query<'w, 's, (), With<crate::game::movement::CaInfected>>,
    /// The bots, whose shots the server traces on its own pose.
    bots: Query<'w, 's, (), With<crate::core::Bot>>,
    /// Mercenaries, whose rounds deal body damage wherever they land
    /// (bots_objectives.md 3.3 Correction 1: PvE AI has no zone factor).
    mercs: Query<'w, 's, (), With<crate::game::rules::mercs::CaMerc>>,
    teams: Query<'w, 's, &'static crate::core::Team>,
    figures: Query<'w, 's, Option<&'static crate::game::movement::CaCharacter>>,
    knockback: Query<'w, 's, &'static crate::game::movement::HitKnockback>,
    now: Res<'w, SimClock>,
    commands: Commands<'w, 's>,
    /// Colliders' own surfaces: props' and Combat Arms' glass sets'
    /// (`PropSurface`), brush entities' (`map::BrushSurface`).
    surfaces: (Query<'w, 's, &'static PropSurface>, Query<'w, 's, &'static crate::map::BrushSurface>),
    colliders: Query<'w, 's, &'static ColliderOf>,
    bodies: Query<'w, 's, (&'static RigidBody, Option<&'static ColliderAabb>)>,
    table: Res<'w, CaSurfaces>,
    /// The world's surfaces (walls without a `PropSurface`), another
    /// game's surface names as Combat Arms' (a mashup's map), what
    /// takes damage that isn't a character (map breakables, props), and
    /// the map's world models (shot through by their thickness alone).
    ground: (
        Option<Res<'w, SurfaceGrid>>,
        Option<Res<'w, SurfaceNames>>,
        Query<'w, 's, (), (With<Damageable>, Without<Intent>)>,
        Query<'w, 's, (), With<crate::map::MapBrushEntity>>,
    ),
    /// Live grenades, which rounds hit (grenades.md 13), and special
    /// weapons' objects (rockets, mines: special_weapons.md 1.4).
    grenades: Query<'w, 's, (Entity, &'static crate::game::grenades::CaGrenade)>,
    objects: Query<'w, 's, (Entity, &'static crate::game::special::CaTarget)>,
    damage: MessageWriter<'w, Damage>,
    /// The hit's full region beside its damage (`core::RegionHit`).
    regions: Option<ResMut<'w, Messages<crate::core::RegionHit>>>,
    /// Rounds' pushes on physics objects (`weapon::impact`).
    impulses: MessageWriter<'w, crate::weapon::impact::BulletImpulse>,
    rules: DamageRules<'w, 's>,
    /// Spawn-protected characters (`core::SpawnProtected`, a party's): a
    /// hit on them shows nothing (`weapon::impact::hide_protected`).
    spawn_protected: Query<'w, 's, &'static crate::core::SpawnProtected>,
    /// The mode and friendly fire (game_modes.md S8: a teammate's round is
    /// scaled before armour and the zone factor).
    settings: Option<Res<'w, crate::game::rules::CaRuleSettings>>,
    first: Res<'w, FirstTimePredicted>,
    role: Option<Res<'w, NetRole>>,
    playback: Option<Res<'w, crate::core::Playback>>,
    /// The attachments' target groups (an AI victim) and a forced draw for
    /// their probabilities.
    attach: (Query<'w, 's, &'static CaNpc>, Res<'w, CaSpecialRoll>),
    /// AI damage (Corrections 7).
    ai: AiHits<'w, 's>,
    /// Damage the shooter's gear adds (Fighting Chance, set items;
    /// gear_perks.md 11, 12, 15) and the weapons' ids.
    dealt: Query<'w, 's, &'static crate::game::gear::CaGearEffects>,
    gear_data: Option<Res<'w, crate::game::gear::CaGearData>>,
    ids: Query<'w, 's, &'static crate::weapon::Weapon>,
    /// Lag compensation (a network server): each shooter's view tick,
    /// the characters' kept hit volumes, `sv_unlag`.
    lag: lagcomp::LagComp<'w, 's>,
}

impl Delivery<'_, '_> {
    fn authoritative(&self) -> bool {
        self.role.as_deref().is_none_or(|r| *r != NetRole::Client) && self.playback.is_none()
    }

    /// Where the other characters were when `owner` fired, as its client
    /// drew them (lag compensation, `weapon::lagcomp`): a network server's
    /// remote player's shot with `sv_unlag` on; else none (the present).
    fn rewind(&mut self, owner: Entity, server: bool) -> Vec<lagcomp::Rewound> {
        self.lag.rewind(owner, server, None)
    }

    /// Every living character but `owner`, as the ray sees it: where it
    /// is, or where `rewound` puts it (its box and look then); `yaws`:
    /// each character's body yaw (`body_yaw`; none: its view's).
    fn bodies(&self, owner: Entity, rewound: &[lagcomp::Rewound], yaws: &HashMap<Entity, f32>) -> Vec<Body> {
        // A bot's shot is traced on the server's pose (characters.md 8,
        // Open question 5: no `Torso` pitch there).
        let server = self.bots.contains(owner);
        self.characters
            .iter()
            .filter(|(e, .., h, _, _, _)| *e != owner && h.current > 0.0)
            // The spawn-protected in their bullets window aren't there
            // (Chimera's `core::SpawnProtected::passes_attacks`, a party's):
            // the round goes on to what is behind.
            .filter(|(e, ..)| !self.spawn_protected.get(*e).is_ok_and(|p| p.passes_attacks(self.now.now)))
            .filter_map(|(entity, t, intent, state, _, aabb, spheres, (pose, boxes, identity))| {
                let data = spheres.map_or(*MALE, |s| s.0);
                let rewound = rewound.iter().find(|r| r.entity == entity);
                // Another game's body with its own hitboxes (then, when
                // rewound): those, as its game made them.
                let theirs = match rewound {
                    Some(r) if !crate::slots::GameIdentity::allows(identity, crate::game::GAME) => {
                        r.hitboxes.as_ref().map(|h| {
                            let (feet, _) = r.frame();
                            body_shapes(&h.get().0, feet, Quat::from_rotation_y(r.yaw))
                        })
                    }
                    Some(_) => None,
                    None => their_shapes(t, intent, aabb, identity, boxes),
                };
                // Ours rewound: the spheres as they were posed on the tick
                // the shooter saw (the kept hitboxes, `characters::
                // hitboxes`), not this tick's pose at that place.
                let kept = rewound
                    .filter(|_| theirs.is_none())
                    .and_then(|r| r.hitboxes.clone().map(|h| (h, r.frame().0, Quat::from_rotation_y(r.yaw))));
                // The body's twist from its view (the legs lag the mouse).
                let twist = yaws.get(&entity).map_or(0.0, |b| wrap_angle(b - intent.yaw));
                // A body model's own pose; else the spec's skeleton.
                let modelled = pose.filter(|p| p.modelled_on(data.first().map_or(0, |s| s.skeleton))).cloned();
                let pitch = -intent.pitch;
                let (origin, min, max, body, spheres): (_, _, _, _, Box<dyn FnOnce() -> Vec<(Vec3, f32)> + Send>) = match rewound {
                    Some(r) => {
                        let b = r.aabb();
                        let turn = Quat::from_rotation_y(r.yaw);
                        let (origin, (min, max)) = (r.origin, (b.min, b.max));
                        let (yaw, view) = (r.yaw, intent.yaw);
                        let spheres = move || match modelled {
                            Some(p) => posed_geometry_by(origin, min, yaw + (p.legs_yaw() - view), &p, data, server),
                            None => geometry_at_by(origin, turn, min, max, data, pitch, server).2,
                        };
                        (r.origin, min, max, r.yaw + twist, Box::new(spheres))
                    }
                    None => {
                        let (min, max) = body_box(t, state, aabb)?;
                        let (origin, look) = (t.translation, intent.yaw_rotation());
                        let spheres = move || match modelled {
                            Some(p) => posed_geometry_by(origin, min, p.legs_yaw(), &p, data, server),
                            None => geometry_at_by(origin, look, min, max, data, pitch, server).2,
                        };
                        (t.translation, min, max, intent.yaw + twist, Box::new(spheres))
                    }
                };
                let turn = Quat::from_rotation_y(body);
                // Another game's shapes may poke out of the hull (a taller
                // model's head): the box holds them, so they're reached.
                // So may an NPC or boss body's spheres (a hit table past the
                // player's two, characters.md 13.6: the hull is the
                // player's, Open question 35).
                let held = theirs.is_some() || data.first().is_some_and(|s| s.skeleton >= 2);
                let shapes = match (theirs, kept) {
                    (Some(t), _) => Later::ready(t),
                    (None, Some((h, feet, turn))) => Later::new(move || body_shapes(&h.get().0, feet, turn)),
                    (None, None) => Later::new(move || sphere_shapes(data, &spheres())),
                };
                let (min, max) = if held {
                    box_holding(min, max, &shapes)
                } else {
                    (min, max)
                };
                Some(Body {
                    entity,
                    origin,
                    min,
                    max,
                    forward: turn * Vec3::NEG_Z,
                    right: turn * Vec3::X,
                    shapes,
                })
            })
            .collect()
    }

    /// S8's factor on a round from `a` to `b` (Open question 26: before
    /// armour and the zone factor): `FF_SCALE` between teammates of a
    /// team mode that scales friendly fire, with it on; else 1.
    fn friendly_scale(&self, a: Entity, b: Entity) -> f32 {
        let Some(s) = self.settings.as_deref() else { return 1.0 };
        let scaled = s.friendly_fire != 0 && crate::game::rules::mode_by_key(&s.mode).is_some_and(|m| m.ff_scaled);
        let mates = a != b
            && matches!((self.teams.get(a), self.teams.get(b)), (Ok(x), Ok(y)) if x == y && matches!(x.0, 1 | 2));
        if scaled && mates { crate::game::rules::scoring::FF_SCALE } else { 1.0 }
    }

    /// Both on one team (neither without one).
    fn same_team(&self, a: Entity, b: Entity) -> bool {
        matches!((self.teams.get(a), self.teams.get(b)), (Ok(x), Ok(y)) if x == y)
    }

    fn body(&self, collider: Entity) -> Entity {
        self.colliders.get(collider).map_or(collider, |c| c.body)
    }

    /// The name of the surface a round hit (`CaSurfaces`' key): the
    /// collider's own (`effects::owned_surface`: a prop's, a glass set's, a
    /// brush entity's such as a CS:S window), else the world's surface
    /// type at `point`; another game's names (a mashup's map) as Combat
    /// Arms' (`map::SurfaceNames`).
    fn surface_name(&self, collider: Entity, point: Vec3) -> Option<String> {
        let names = self.ground.1.as_deref();
        let tables = crate::game::effects::data::CaFxTables::built_in();
        let (props, brushes) = &self.surfaces;
        if let Some(p) = crate::game::effects::owned_surface(collider, props, brushes, &self.colliders) {
            let p = names.map_or(p, |n| n.resolve(p));
            // Combat Arms' own world models name their type's number
            // (`library::tag_surfaces`).
            return match p.parse::<u16>() {
                Ok(ty) => Some(tables.surface.iter().find(|s| s.id == ty)?.name.clone()),
                Err(_) => Some(p.to_string()),
            };
        }
        let ty = crate::game::effects::world_surface_named(self.ground.0.as_deref(), names, point);
        Some(tables.surface.iter().find(|s| s.id == ty)?.name.clone())
    }

    /// Whether a collider has a surface of its own (a prop's, a brush
    /// entity's, a glass set's), not the world's there.
    fn owns_surface(&self, collider: Entity) -> bool {
        let (props, brushes) = &self.surfaces;
        crate::game::effects::owned_surface(collider, props, brushes, &self.colliders).is_some()
    }

    /// How a round passes what it hit (`surface_name` in `CaSurfaces`).
    /// None: it stops there.
    fn surface_at(&self, collider: Entity, point: Vec3) -> Option<SurfacePass> {
        self.table.get(&self.surface_name(collider, point)?)
    }
}

/// A character's body yaw (weapon_firing.md Corrections 6, inventory.md
/// Corrections 2): its view yaw `view` minus the upper body's twist, as
/// its weapon's spread keeps it (`CaSpread::body_yaw`; none: the view's).
pub fn body_yaw(view: f32, kept: Option<f32>) -> f32 {
    kept.unwrap_or(view)
}

/// A Combat Arms body's hit spheres (`data`, at `spheres`: world centres
/// and radii) as Chimera's hit shapes (`core::Hitbox`): each sphere's
/// region (`HitSphere::region`), `Node#Priority` and zone factor (the
/// body's own scale).
pub fn sphere_shapes(data: &[HitSphere], spheres: &[(Vec3, f32)]) -> crate::core::Hitboxes {
    crate::core::Hitboxes(
        data.iter()
            .zip(spheres)
            .map(|(s, (c, r))| crate::core::Hitbox {
                priority: s.priority,
                scale: Some(s.factor),
                ..crate::core::Hitbox::sphere(*c, *r, s.region())
            })
            .collect(),
    )
}

/// Another game's body's hitboxes (character space: `local`) in the
/// world, its feet at `feet` and turned by `turn`.
pub fn body_shapes(local: &[crate::core::Hitbox], feet: Vec3, turn: Quat) -> crate::core::Hitboxes {
    crate::core::Hitboxes(
        local
            .iter()
            .map(|h| crate::core::Hitbox {
                center: feet + turn * h.center,
                rotation: turn * h.rotation,
                ..*h
            })
            .collect(),
    )
}

/// A character's hit shapes in the world when its body is another game's
/// (its identity isn't Combat Arms' and that game's body gave it
/// `core::Hitboxes`): those, its feet at its box's bottom, turned by its
/// look's yaw (as every game places them). None: a body of ours, whose
/// shapes are our skeleton's spheres.
pub fn their_shapes(
    t: &Transform,
    intent: &Intent,
    aabb: Option<&ColliderAabb>,
    identity: Option<&crate::slots::GameIdentity>,
    hitboxes: Option<&crate::core::Hitboxes>,
) -> Option<crate::core::Hitboxes> {
    if crate::slots::GameIdentity::allows(identity, crate::game::GAME) {
        return None;
    }
    let feet = aabb.map_or(t.translation, |b| t.translation.with_y(b.min.y));
    hitboxes.map(|h| body_shapes(&h.0, feet, intent.yaw_rotation()))
}

/// A body's box (world min, max) widened to hold its hit shapes (world),
/// so a shape poking out of the hull (another game's taller model's head)
/// is still reached through the box tests.
pub fn box_holding(min: Vec3, max: Vec3, shapes: &crate::core::Hitboxes) -> (Vec3, Vec3) {
    shapes.bounds().map_or((min, max), |(a, b)| (min.min(a), max.max(b)))
}

/// Combat Arms' zone factor for a region of a body that gives none of its
/// own (another game's: its hitboxes have no scale): the male skeleton's
/// first sphere of that region (data/characters.toml), else of its
/// hitgroup's region, else 1.
pub fn zone_factor(region: crate::core::BodyRegion) -> f32 {
    // The male spheres' regions as the character data keeps them (made
    // once: a sphere's region is read from its node's name, a lower-cased
    // copy and several searches, and every other game's hitbox asks on
    // each look at the bodies).
    let zones = &characters::records::builtin().male_zones;
    let of = |r: crate::core::BodyRegion| zones.iter().find(|(region, _)| *region == r).map(|(_, factor)| *factor);
    of(region)
        .or_else(|| of(crate::core::BodyRegion::from_hitgroup(region.hitgroup())))
        .unwrap_or(1.0)
}

/// A region's armour location (`BodyPart`'s, Combat Arms' five).
pub fn location_of(region: crate::core::BodyRegion) -> Location {
    match region.part() {
        crate::core::BodyPart::Head => Location::Head,
        crate::core::BodyPart::Torso => Location::Torso,
        crate::core::BodyPart::Arm => Location::Arm,
        crate::core::BodyPart::Leg => Location::Leg,
        crate::core::BodyPart::Groin => Location::Groin,
    }
}

/// A character's box (world min, max: its movement's hull, else its
/// collider's bounds) and its spheres `data` in the world (centre,
/// radius): the spec's skeleton (`characters::skeleton::fallback`)
/// crouched by the box's height and pitched by the look.
pub fn body_geometry(
    t: &Transform,
    intent: &Intent,
    state: &MovementState,
    aabb: Option<&ColliderAabb>,
    data: &[HitSphere],
) -> Option<(Vec3, Vec3, Vec<(Vec3, f32)>)> {
    let (min, max) = if state.hull_max != state.hull_min {
        (t.translation + state.hull_min, t.translation + state.hull_max)
    } else {
        let b = aabb?;
        (b.min, b.max)
    };
    Some(geometry_at(t.translation, intent.yaw_rotation(), min, max, data, -intent.pitch))
}

/// `body_geometry` with the server's aim shares (`server`: no `Torso`
/// pitch, characters.md 8) for the hits the server traces itself.
pub fn body_geometry_by(
    t: &Transform,
    intent: &Intent,
    state: &MovementState,
    aabb: Option<&ColliderAabb>,
    data: &[HitSphere],
    server: bool,
) -> Option<(Vec3, Vec3, Vec<(Vec3, f32)>)> {
    let (min, max) = if state.hull_max != state.hull_min {
        (t.translation + state.hull_min, t.translation + state.hull_max)
    } else {
        let b = aabb?;
        (b.min, b.max)
    };
    Some(geometry_at_by(t.translation, intent.yaw_rotation(), min, max, data, -intent.pitch, server))
}

/// A character's box (world min, max) as `body_geometry` takes it: its
/// movement's hull, else its collider's bounds; None without either.
pub fn body_box(t: &Transform, state: &MovementState, aabb: Option<&ColliderAabb>) -> Option<(Vec3, Vec3)> {
    if state.hull_max != state.hull_min {
        Some((t.translation + state.hull_min, t.translation + state.hull_max))
    } else {
        aabb.map(|b| (b.min, b.max))
    }
}

/// A character's spheres `data` in the world for its box `min..max`
/// (world) around `origin`, turned by `turn`, with the third-person aim
/// pitch `pitch` (radians, positive down): (min, max, spheres). The
/// skeleton is squashed by the box's height over the standing one.
pub fn geometry_at(origin: Vec3, turn: Quat, min: Vec3, max: Vec3, data: &[HitSphere], pitch: f32) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    geometry_at_by(origin, turn, min, max, data, pitch, false)
}

/// `geometry_at` with the server's aim shares when `server`.
pub fn geometry_at_by(
    origin: Vec3,
    turn: Quat,
    min: Vec3,
    max: Vec3,
    data: &[HitSphere],
    pitch: f32,
    server: bool,
) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    let unit = crate::game::movement::movement_data().meters_per_unit;
    let squash = ((max.y - min.y) / (characters::STAND_HEIGHT * unit)).max(0.0);
    let skel = characters::records::builtin().skeleton(data.first().map_or(0, |s| s.skeleton));
    let mut posed = characters::skeleton::fallback(skel, squash);
    posed.aim(&skel.aim, pitch, 0.0, server);
    let feet = origin.with_y(min.y);
    let spheres = data
        .iter()
        .map(|s| {
            // A sphere the stand-in skeleton has no place for (an install
            // table's name it lacks) can't be hit.
            match posed.sphere(s.name) {
                Some(p) => (feet + turn * (characters::skeleton::to_body(p, posed.feet) * unit), s.radius * unit),
                None => (feet, 0.0),
            }
        })
        .collect();
    (min, max, spheres)
}

/// A character's spheres from its posed body model (`characters::CaPose`)
/// for its box `min..max` around `origin`, its legs turned to `yaw`.
pub fn posed_geometry(origin: Vec3, min: Vec3, yaw: f32, pose: &characters::CaPose, data: &[HitSphere]) -> Vec<(Vec3, f32)> {
    posed_geometry_by(origin, min, yaw, pose, data, false)
}

/// `posed_geometry` on the server's pose (`CaPose::server_spheres`) when
/// `server`.
pub fn posed_geometry_by(
    origin: Vec3,
    min: Vec3,
    yaw: f32,
    pose: &characters::CaPose,
    data: &[HitSphere],
    server: bool,
) -> Vec<(Vec3, f32)> {
    let unit = crate::game::movement::movement_data().meters_per_unit;
    let feet = origin.with_y(min.y);
    let turn = Quat::from_rotation_y(yaw);
    let centres: &[Option<Vec3>] = match server.then(|| pose.server_spheres()).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => &pose.spheres,
    };
    data.iter()
        .enumerate()
        .map(|(i, s)| match centres.get(i).copied().flatten() {
            Some(p) => (feet + turn * (p * unit), s.radius * unit),
            None => (feet, 0.0),
        })
        .collect()
}

/// Where a ray enters an axis-aligned box (distance along the unit `dir`,
/// clamped at 0), if within `limit`: Chimera's (lift C1).
pub use crate::weapon::projectile::ray_box as box_entry;

/// The direction after a shoot-through surface (Corrections 9): `dir`
/// plus `k_r` / 1000 and `k_u` / 1000 on the right and up axes of a look
/// rotation along it with world up, normalised.
pub fn reperturb(dir: Vec3, k_r: i32, k_u: i32) -> Vec3 {
    let right = Vec3::Y.cross(dir).try_normalize().unwrap_or(Vec3::X);
    let up = dir.cross(right);
    (dir + right * (k_r as f32 / 1000.0) + up * (k_u as f32 / 1000.0)).normalize()
}

/// A whole number drawn uniformly from {-p..p} with `u` in [0, 1).
pub fn whole_draw(u: f32, p: i32) -> i32 {
    let p = p.max(0);
    ((u * (2 * p + 1) as f32) as i32).min(2 * p) - p
}

/// Whether a ray of the server's spacing check (Corrections 3) passes:
/// a shot of `rays` rays at `now` (s) whose last ray only is checked
/// against `last` (the last ray that passed or wasn't checked) and
/// `delay` (s), strictly in whole ms; every other ray stamps the clock.
/// Returns, per ray, whether it lands, and updates `last`.
pub fn spacing(rays: u32, now: f64, delay: f64, last: &mut Option<f64>) -> Vec<bool> {
    (0..rays)
        .map(|i| {
            if i + 1 == rays {
                let early = last.is_some_and(|t| ((now - t) * 1000.0 + 1e-6).floor() < delay * 1000.0 - 1e-6);
                if early {
                    return false;
                }
            }
            *last = Some(now);
            true
        })
        .collect()
}

/// After the frame: deliver each round, return the recoil, step the
/// spread, kick the view.
#[allow(clippy::type_complexity)]
pub(super) fn after_shots(
    mut events: ParamSet<(MessageReader<WeaponEvent>, MessageWriter<WeaponEvent>)>,
    mut owners: Query<
        (
            Entity,
            &Inventory,
            &Intent,
            &MovementState,
            &Velocity,
            Option<&mut ViewPunch>,
            Option<&crate::game::movement::CaMovement>,
            Option<&mut CaKick>,
            Option<&crate::weapon::impact::RecoilGame>,
            Option<&mut ViewBackPush>,
            Option<&crate::game::damage_fx::CaProgress>,
        ),
    >,
    mut weapons: Query<(
        Option<&mut CaDamage>,
        Option<&mut CaSpread>,
        Option<&CaRecoil>,
        Option<&Hitscan>,
        Option<&AltModes>,
        Option<&Burst>,
        Option<&Zoom>,
        Option<&CaZoom>,
    )>,
    mut delivery: Delivery,
    exact: Query<(), With<ExactAim>>,
    clock: Res<SimClock>,
    scales: Res<SpreadScales>,
    (options, carry_over, server_settings): (Res<SpreadOptions>, Res<PelletCarryOver>, Res<CaServerSettings>),
    fx_table: Option<Res<crate::game::damage_fx::CaDamageFxTable>>,
    mut commands: Commands,
) {
    let now = clock.now;
    let all: Vec<WeaponEvent> = events.p0().read().cloned().collect();
    // The server's reload windows (weapon_classes.md Corrections 2).
    if delivery.authoritative() && delivery.first.0 {
        let started: Vec<Entity> = all
            .iter()
            .filter(|e| !e.replay && matches!(e.kind, WeaponEventKind::ReloadStarted))
            .map(|e| e.weapon)
            .collect();
        for w in started {
            if let Ok((Some(mut dmg), ..)) = weapons.get_mut(w) {
                let rate = 1.0 + dmg.mods.reload_rate as f64;
                let mark = server_reload_mark(dmg.reload_third_person as f64, rate, server_settings.reload_check_time);
                dmg.reload_mark = Some(now + mark);
            }
        }
    }
    let rounds: Vec<Round> = all
        .iter()
        .filter_map(|e| match e.kind {

            WeaponEventKind::Fired {
                origin,
                yaw,
                pitch,
                seed,
                mode,
                ..
            } => Some(Round {
                owner: e.owner,
                weapon: e.weapon,
                origin,
                yaw,
                pitch,
                seed,
                mode,
                replay: e.replay,
            }),
            _ => None,
        })
        .collect();

    // Each character's body yaw, for the behind test.
    let yaws: HashMap<Entity, f32> = if rounds.is_empty() {
        HashMap::new()
    } else {
        owners
            .iter()
            .map(|(e, inv, intent, ..)| {
                let kept = inv.active.and_then(|w| weapons.get(w).ok()).and_then(|w| w.1.and_then(|s| s.body_yaw));
                (e, body_yaw(intent.yaw, kept))
            })
            .collect()
    };

    // Deliver, with the cone the meters give before this tick's steps.
    let mut out: Vec<WeaponEvent> = Vec::new();
    for r in &rounds {
        let Ok((Some(mut dmg), spread, _, scan, _, burst, zoom, ca_zoom)) = weapons.get_mut(r.weapon) else {
            continue;
        };
        let mode = mode_of(r.mode, burst, zoom, ca_zoom);
        let exact_aim = exact.contains(r.owner);
        let half = if exact_aim {
            0.0
        } else {
            spread.as_ref().map_or(0.0, |s| s.cone(mode)) / 1000.0
        };
        let range = scan.map_or(0.0, |s| s.range);
        let server = delivery.authoritative() && delivery.first.0 && !r.replay;
        // The rays (attachments.md Corrections 1): the client sends n +
        // floor(n v) with the magazine part's `VectorsPerRoundPer`, the
        // server expects the muzzle part's; when they differ it ignores
        // the whole shot.
        let n = if exact_aim { 1 } else { scan.map_or(1, |s| s.pellets.max(1)) };
        let (sent, expected) = if exact_aim {
            (1, 1)
        } else {
            (dmg.mods.rays(n, dmg.mods.pellets_client), dmg.mods.rays(n, dmg.mods.pellets_server))
        };
        // The server's spacing (Corrections 3): checked on the last ray
        // only; every earlier ray stamps the clock with the shot's time.
        // A shot during the server's reload window is dropped (no server
        // ammo spent, Corrections 2).
        let reloading = dmg.reload_mark.is_some_and(|m| now + 1e-9 < m);
        if !reloading && dmg.reload_mark.is_some() && server {
            dmg.reload_mark = None;
        }
        let lands = if server && sent == expected && !reloading {
            spacing(sent, now, dmg.fire_delay, &mut dmg.last_round)
        } else {
            vec![false; sent as usize]
        };

        // The attachments (attachments.md, "Damage" and "Ray length"): the
        // base damage x (1 + sum Instdamage); the shooter's ray (the
        // client test) Range + trunc(Range x sum RangePer), the server's
        // own walk Range x (1 + the sight's).
        let mut eff = dmg.clone();
        eff.damage *= 1.0 + eff.mods.damage;
        let units = (range / eff.unit).round() as f64;
        let client_range = if eff.mods.range_per != 0.0 {
            eff.mods.client_range(units) as f32 * eff.unit
        } else {
            range
        };
        let server_range = if eff.mods.sight_range_per != 0.0 {
            eff.mods.server_range(units) as f32 * eff.unit
        } else {
            range
        };
        let dmg = &eff;
        let aim = aim_of(r.yaw, r.pitch);
        let rewound = delivery.rewind(r.owner, server);
        let bodies = delivery.bodies(r.owner, &rewound, &yaws);
        // Weapon_classes.md 5.7: a pellet that hits no one reports the
        // shot's most recent real hit.
        let mut carry = CarryOver::new(carry_over.0);
        for (pellet, land) in lands.into_iter().enumerate() {
            let mut rng = Rng::new(r.seed.wrapping_add(1 + pellet as u32));
            let (u1, u2) = (rng.next(), rng.next());
            let dir = square_dir(aim, half, u1, u2);
            let real = client_test(&mut delivery, r, dir, client_range, &bodies, &mut out);
            let report = carry.pellet(real).or(real);
            // The reported hit: the roll, from behind (the body's), the
            // band at the reported point; then the server's own walk.
            let u = rng.next();
            let mut first = None;
            if let Some(c) = report {
                let b = &bodies[c.body];
                let behind = is_behind(b.forward, b.right, c.dir);
                let d = base_damage(dmg, u, behind, c.distance);
                if land {
                    hurt(&mut delivery, dmg, r, b, c.sphere, d, c.point, c.dir, &mut out);
                }
                first = Some((d, c.distance));
            }
            let walk = Walk {
                dir,
                range: server_range,
                lands: land,
                skip_first: report.is_some(),
                first,
            };
            server_walk(&mut delivery, dmg, r, walk, &bodies, &mut rng, &mut out);
        }
    }

    // The recoil returns, the spread steps for each owner's active
    // weapon, then the kicks.
    let dt = clock.dt();
    // The back push's offset (units) as the camera's metres.
    let back_metres = BACK_PUSH_SCALE * crate::game::inventory::UNIT;
    for (owner, inv, intent, state, vel, mut punch, ca_move, kick, recoil_game, push, progress) in &mut owners {
        let active = inv.active.and_then(|a| weapons.get_mut(a).ok().map(|w| (a, w)));
        let (recoil, mode, burst_mode) = match &active {
            Some((_, (_, _, recoil, _, modes, burst, zoom, ca_zoom))) => {
                let current = modes.map_or(0, |m| m.current);
                (
                    recoil.cloned().unwrap_or_else(|| DEFAULT_RECOIL.clone()),
                    mode_of(current, *burst, *zoom, *ca_zoom),
                    fire_mode(current, *burst, *zoom) == FireMode::Burst,
                )
            }
            None => (DEFAULT_RECOIL.clone(), FireMode::Normal, false),
        };
        let (burst_part, zoom_part) = match &active {
            Some((_, w)) => (w.5.cloned(), w.6.cloned()),
            None => (None, None),
        };
        let mut k = kick.as_deref().cloned().unwrap_or_default();
        // Holding another game's weapon (the gun game's swap): that game's
        // recoil owns the view punch (`weapon::impact::RecoilGame`, which
        // zeroed it at the switch); this game's kick ends, nothing returns.
        // (A weapon with Combat Arms' recoil data is its own whatever its
        // id: tests' `test:` guns.)
        let ours = crate::weapon::impact::RecoilGame::is(recoil_game, crate::game::GAME)
            || active.as_ref().is_some_and(|(_, w)| w.2.is_some());
        let mut back = push.as_deref().map_or(0.0, |p| p.0) / back_metres;
        if !ours {
            k.pitch = 0.0;
            k.yaw = 0.0;
            back = 0.0;
        }
        // The return, with the data of the weapon in hand (Corrections 1).
        if ours && (k.pitch > 0.0 || k.yaw != 0.0) {
            let (down, side) = recoil.recover(&mut k, dt, now, burst_mode);
            add_punch(&mut punch, &mut commands, owner, Vec3::new(-down, side, 0.0));
        }
        if back != 0.0 {
            recoil.back_recover(&mut back, dt);
        }
        let mine: Vec<&Round> = active
            .as_ref()
            .map(|(a, _)| rounds.iter().filter(|r| r.owner == owner && r.weapon == *a).collect())
            .unwrap_or_default();
        if let Some((_, (_, Some(mut s), ..))) = active {
            let tick = SpreadTick {
                intent,
                state,
                movement: ca_move,
                velocity: vel.0,
                punch: punch.as_deref(),
                mode,
                fired: !mine.is_empty(),
                forced: progress
                    .zip(fx_table.as_deref())
                    .is_some_and(|(p, t)| crate::game::damage_fx::forces_spread(t, p)),
                now,
                dt,
            };
            step_spread(&mut s, &scales, &options, &tick);
        }
        if ours && !exact.contains(owner) {
            for (i, r) in mine.iter().enumerate() {
                let burst = fire_mode(r.mode, burst_part.as_ref(), zoom_part.as_ref()) == FireMode::Burst;
                let up = recoil.kick(&mut k, burst, now);

                let roll = shared_random(intent.command, "combat_arms:wag", i as i32).next();
                let side = recoil.wag(&mut k, roll);
                add_punch(&mut punch, &mut commands, owner, Vec3::new(up, side, 0.0));
                recoil.back_kick(&mut back);
            }
        }
        let back = ViewBackPush(back * back_metres);
        match push {
            Some(mut have) => {
                if *have != back {
                    *have = back;
                }
            }
            None => {
                if back.0 != 0.0 {
                    commands.entity(owner).insert(back);
                }
            }
        }
        match kick {
            Some(mut have) => {
                if *have != k {
                    *have = k;
                }
            }
            None => {
                if k != CaKick::default() {
                    commands.entity(owner).insert(k);
                }
            }
        }
    }
    // A spawn-protected character's hits show nothing (Chimera's
    // `SpawnProtected`; plain Combat Arms' own protection is `CaProtected`,
    // whose blood the effects leave out).
    let at = delivery.now.now;
    let hides = |t: Entity| delivery.spawn_protected.get(t).is_ok_and(|p| p.hides_hits(at));
    let out: Vec<WeaponEvent> = out
        .into_iter()
        .filter_map(|e| {
            let kind = crate::weapon::impact::hide_protected(e.kind, hides)?;
            Some(WeaponEvent { kind, ..e })
        })
        .collect();
    let mut writer = events.p1();
    for e in out {
        writer.write(e);
    }
}

/// Add `delta` to `owner`'s view punch (giving it one).
pub(super) fn add_punch(punch: &mut Option<Mut<ViewPunch>>, commands: &mut Commands, owner: Entity, delta: Vec3) {
    match punch.as_mut() {
        Some(p) => p.0 += delta,
        None => {
            commands.entity(owner).insert(ViewPunch(delta));
        }
    }
}

/// The movement fraction mu: the speed over the tick's speed limit, at
/// most 1 (no limit: 1).
pub fn movement_fraction(speed: f32, limit: f32) -> f32 {
    if limit > 0.0 { (speed / limit).min(1.0) } else { 1.0 }
}

/// The turn term of each of a tick's `n` sub-steps for a body turn of
/// `turned` radians (Q13): all of it on the first, 0 on the rest.
pub fn substep_turns(scales: &SpreadScales, turned: f32, n: u32) -> Vec<f32> {
    (0..n).map(|i| if i == 0 { CaSpread::turn(scales, 0.0, turned) } else { 0.0 }).collect()
}

/// What one tick of the spread meters reads of the owner.
pub(super) struct SpreadTick<'a> {
    intent: &'a Intent,
    state: &'a MovementState,
    /// Combat Arms' movement, when the owner has it: its run flag, "in a
    /// jump" and speed limit.
    movement: Option<&'a crate::game::movement::CaMovement>,
    velocity: Vec3,
    punch: Option<&'a ViewPunch>,
    mode: FireMode,
    fired: bool,
    /// A poison or stun screen effect runs while its type deals damage
    /// (effects_sounds.md Follow-up (2026-11-14) 4, `damage_fx::
    /// forces_spread`): mu S is 1.
    forced: bool,
    now: f64,
    dt: f32,
}

/// The movement target S in a jump (Corrections 12): the zoomed
/// `ZoomedJumpPerturb` only zoomed with the run flag off and non-zero,
/// else the plain `JumpPerturb`.
pub fn jump_target(params: &SpreadParams, mode: FireMode, run: bool) -> f32 {
    let z = params.zoomed_state.jump;
    if mode == FireMode::Zoomed && !run && z != 0.0 { z } else { params.state.jump }
}

/// One tick of a weapon's spread meters (the spec's step 8).
pub(super) fn step_spread(s: &mut CaSpread, scales: &SpreadScales, options: &SpreadOptions, k: &SpreadTick) {
    let (intent, state) = (k.intent, k.state);
    // Airborne from a jump (Q12): movement.md's "in a jump"; without that
    // movement, left the ground with the jump key held, until landing.
    if state.on_ground {
        s.jumped = false;
    } else if s.was_on_ground {
        s.jumped = intent.jump;
    }
    s.was_on_ground = state.on_ground;
    let jumping = k.movement.map_or(s.jumped && !state.on_ground, |m| m.in_jump);
    let cone = s.cone_params(k.mode);
    let st = s.states(k.mode);
    // "No spread" holds both meters at 0: the plain `MaxPerturb` 0
    // (Corrections 12), whatever the mode's.
    if s.params.cone.max == 0 {
        s.fire = 0.0;
        s.movement = 0.0;
        return;
    }
    let keys = intent.move_axis != Vec2::ZERO || intent.opposed.any();
    let cap = match (keys, state.crouching) {
        (true, true) => st.fire_move_duck,
        (true, false) => st.fire_move,
        (false, true) => st.fire_duck,
        (false, false) => st.fire,
    };
    // The run flag (Q6): not crouched and not walking, at rest too.
    let run = k.movement.map_or(!intent.walk && !state.crouching, |m| m.run);
    let target = if state.on_ladder {
        st.run
    } else if jumping {
        jump_target(&s.params, k.mode, run)
    } else if run {
        st.run
    } else {
        st.walk
    };
    // mu: against the tick's speed limit, penalties included (Q6).
    let vmax = match k.movement {
        _ if state.on_ladder => LADDER_SPEED,
        Some(m) => m.speed_limit,
        None if state.crouching => CROUCH_SPEED,
        None if run => RUN_SPEED,
        None => WALK_SPEED,
    };
    let mu = movement_fraction(k.velocity.length() / s.unit, vmax);
    let mu_s = if k.forced {
        1.0
    } else if state.on_ladder {
        target
    } else {
        mu * target
    };
    s.clock += k.dt as f64;
    let mut n = ((s.clock / SPREAD_SUBSTEP + 1e-9).floor() as u32).min(SPREAD_SUBSTEP_MAX);
    if n > 0 {
        s.clock = if options.quantised { 0.0 } else { (s.clock - n as f64 * SPREAD_SUBSTEP).max(0.0) };
    } else if k.fired {
        n = 1;
    }
    // The turn (Q13): the body yaw's change, all on the first sub-step.
    let wag = k.punch.map_or(0.0, |p| p.0.y);
    let turned = s.body_turn(intent.yaw, wag, keys);
    // The falling branch's clamp is the plain `JumpPerturb` (Corrections
    // 12).
    let jump_cap = s.params.state.jump;
    for (i, rho) in substep_turns(scales, turned, n).into_iter().enumerate() {
        s.move_substep(scales, cone, target, mu_s, rho, jumping, jump_cap);
        s.fire_substep(scales, cone, k.fired && i == 0, cap, k.now);
    }
}

/// A pellet's real hit on a character in the shooter's own test, the hit
/// it reports (weapon_classes.md 5.7's carry-over reports one too).
#[derive(Clone, Copy, Debug)]
pub(super) struct PelletHit {
    /// Index in the round's bodies.
    body: usize,
    sphere: usize,
    /// The reported point: the widened box's entry (Corrections 8).
    point: Vec3,
    dir: Vec3,
    /// From the round's origin, metres.
    distance: f32,
}

/// The shooter's own test of one ray (the spec's "Client test"): the
/// widened boxes whose spheres the whole segment passes, the nearest
/// before any wall; then the plain test decides what is reported
/// (Corrections 8): the same character at the widened box's entry, or
/// something else whose plane has the victim's origin behind it (no
/// player hit), else still the character. A live grenade in front stops
/// the round. Its `Shot` event; the character hit, if any. Never through
/// anything.
pub(super) fn client_test(w: &mut Delivery, r: &Round, dir: Vec3, range: f32, bodies: &[Body], out: &mut Vec<WeaponEvent>) -> Option<PelletHit> {
    // Special weapons' objects are met by their boxes (`objects`), not by
    // a turret's collider (which other games' rounds meet).
    let excluded: Vec<Entity> = std::iter::once(r.owner)
        .chain(w.characters.iter().map(|(e, ..)| e))
        .chain(w.objects.iter().map(|(e, _)| e))
        .collect();
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(SOLID_LAYERS);
    let from = r.origin;
    let event = |to: Vec3, hit: Option<Entity>, normal: Option<Vec3>| WeaponEvent {
        owner: r.owner,
        weapon: r.weapon,
        kind: WeaponEventKind::Shot { from, to, hit, normal },
        replay: r.replay,
    };
    let Ok(d3) = Dir3::new(dir) else { return None };
    let world = w.spatial.cast_ray(from, d3, range, true, &filter);
    let wall = world.as_ref().map_or(range, |h| h.distance);
    let end = from + dir * range;
    // The nearest character whose widened box the ray enters before the
    // wall and whose spheres the whole segment passes.
    let mut best: Option<(f32, usize, usize)> = None;
    for (bi, b) in bodies.iter().enumerate() {
        let c = (b.min + b.max) / 2.0;
        let half = (b.max - b.min) / 2.0 * Vec3::new(HITBOX_WIDEN, 1.0, HITBOX_WIDEN);
        let Some(t) = box_entry(c - half, c + half, from, dir, wall) else {
            continue;
        };
        if best.is_some_and(|(bt, ..)| bt <= t) {
            continue;
        }
        let Some(si) = b.shapes.by_power(from, end) else {
            continue;
        };
        best = Some((t, bi, si));
    }
    // A live grenade's box before them (grenades.md 13): the round stops
    // on its metal (the server's walk deals its damage).
    let grenade = w
        .grenades
        .iter()
        .filter_map(|(e, g)| {
            let (c, half) = (g.pos * g.unit, Vec3::from(g.params.box_half) * g.unit);
            box_entry(c - half, c + half, from, dir, wall).map(|t| (t, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((t, hit)) = grenade
        && best.is_none_or(|(bt, ..)| t < bt)
    {
        out.push(event(from + dir * t, Some(hit), Some(-dir)));
        return None;
    }
    let world_hit = world.map(|h| (from + dir * h.distance, h.normal, w.body(h.entity)));
    let Some((t_wide, bi, si)) = best else {
        out.push(match world_hit {
            Some((point, normal, body)) => event(point, Some(body), Some(normal)),
            None => event(end, None, None),
        });
        return None;
    };
    let b = &bodies[bi];
    // The plain test: what it meets first.
    let plain = |o: &Body| box_entry(o.min, o.max, from, dir, wall).filter(|_| o.shapes.by_power(from, end).is_some());
    let own = plain(b);
    let other = bodies
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != bi)
        .filter_map(|(i, o)| plain(o).map(|t| (t, i)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let own_first = own.is_some_and(|t| other.is_none_or(|(o, _)| t <= o));
    if !own_first {
        let something = match other {
            Some((t, i)) => Some((from + dir * t, -dir, bodies[i].entity)),
            None => world_hit,
        };
        if let Some((point, normal, thing)) = something
            && (b.origin - point).dot(normal) < 0.0
        {
            out.push(event(point, Some(thing), Some(normal)));
            return None;
        }
    }
    let point = from + dir * t_wide;
    out.push(event(point, Some(b.entity), Some(-dir)));
    Some(PelletHit {
        body: bi,
        sphere: si,
        point,
        dir,
        distance: t_wide,
    })
}

/// How the server walks one ray.
pub(super) struct Walk {
    dir: Vec3,
    /// The server's ray length (m).
    range: f32,
    /// Its damage lands (the server, the shot accepted, the ray not
    /// dropped by the spacing).
    lands: bool,
    /// A player hit was reported: the walk skips the first character box
    /// it meets.
    skip_first: bool,
    /// The reported hit's damage and distance (a second victim's start).
    first: Option<(f32, f32)>,
}

/// The server's own walk of one ray (the spec's "Server" and
/// "Penetration"): from the fire position, at most `SERVER_INTERSECTIONS`
/// intersections (every surface, object or skipped character box counts
/// one); shoot-through surfaces pass (the back-cast from the limit + 1
/// unit, the restart point, the range left from the fire position, the
/// damage dampened, the whole-thousandth re-perturb); a live grenade
/// stops it with its damage; the first character box after the skipped
/// one is hit in the sphere of the highest priority on the line
/// (`priority_sphere`), on its unwidened box, and the walk ends there
/// (none on the line: it goes on 5 units past the box's entry). That
/// character's damage: a reported hit's running value banded again at
/// the reported distance, else base x roll x the band (twice, at the real
/// distance: Q13's recommendation), dampened; no behind bonus.
/// `ShotContinued` events past the first intersection (the shooter's test
/// drew that one).
pub(super) fn server_walk(w: &mut Delivery, dmg: &CaDamage, r: &Round, walk: Walk, bodies: &[Body], rng: &mut Rng, out: &mut Vec<WeaponEvent>) {
    // Special weapons' objects are met by their boxes (`objects`), not by
    // a turret's collider (which other games' rounds meet).
    let excluded: Vec<Entity> = std::iter::once(r.owner)
        .chain(w.characters.iter().map(|(e, ..)| e))
        .chain(w.objects.iter().map(|(e, _)| e))
        .collect();
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(SOLID_LAYERS);
    let unit = dmg.unit;
    let mut from = r.origin;
    let mut dir = walk.dir;
    let mut left = walk.range;
    let mut dampen = 1.0;
    let mut skip: Vec<Entity> = Vec::new();
    let mut to_skip = walk.skip_first;
    for i in 0..SERVER_INTERSECTIONS {
        if left < unit {
            break;
        }
        let Ok(d3) = Dir3::new(dir) else { break };
        let world = w.spatial.cast_ray(from, d3, left, true, &filter);
        let wall = world.as_ref().map_or(left, |h| h.distance);
        let show = i > 0;
        let event = |from: Vec3, to: Vec3, hit: Option<Entity>, normal: Option<Vec3>| WeaponEvent {
            owner: r.owner,
            weapon: r.weapon,
            kind: WeaponEventKind::ShotContinued { from, to, hit, normal },
            replay: r.replay,
        };
        let body = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| !skip.contains(&b.entity))
            .filter_map(|(bi, b)| box_entry(b.min, b.max, from, dir, wall).map(|t| (t, bi)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let grenade = w
            .grenades
            .iter()
            .filter_map(|(e, g)| {
                let (c, half) = (g.pos * g.unit, Vec3::from(g.params.box_half) * g.unit);
                box_entry(c - half, c + half, from, dir, wall).map(|t| (t, e))
            })
            .chain(w.objects.iter().filter_map(|(e, t)| {
                box_entry(t.centre - t.half, t.centre + t.half, from, dir, wall).map(|t| (t, e))
            }))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        // A live grenade's box (grenades.md 13): the round stops on its
        // metal, its damage (with the range band, no zone) off the
        // grenade's hit points.
        if let Some((t, hit)) = grenade
            && body.is_none_or(|(bt, _)| t < bt)
        {
            let point = from + dir * t;
            if show {
                out.push(event(from, point, Some(hit), Some(-dir)));
            }
            let u = rng.next();
            if walk.lands {
                let d = base_damage(dmg, u, false, point.distance(r.origin)) * dampen;
                w.damage.write(Damage {
                    target: hit,
                    attacker: Some(r.owner),
                    amount: d,
                    point,
                    dir,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Bullet,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
            return;
        }
        if let Some((t, bi)) = body {
            let b = &bodies[bi];
            let at = from + dir * t;
            if to_skip {
                // The reported victim's box (or the first met).
                to_skip = false;
                skip.push(b.entity);
                left -= t;
                from = at;
                continue;
            }
            let Some(si) = b.shapes.by_priority(at, dir) else {
                // The line passes it: on, 5 units past its entry.
                skip.push(b.entity);
                let step = SERVER_SKIP * unit;
                left -= t + step;
                from = at + dir * step;
                continue;
            };
            if show {
                out.push(event(from, at, Some(b.entity), Some(-dir)));
            }
            let u = rng.next();
            let d = match walk.first {
                // One behind the reported victim (Q5): its damage banded
                // again at the reported distance.
                Some((prev, distance)) => prev * band_scale(dmg, distance),
                None => {
                    let distance = at.distance(r.origin);
                    base_damage(dmg, u, false, distance) * band_scale(dmg, distance)
                }
            };
            if walk.lands {
                hurt(w, dmg, r, b, si, d * dampen, at, dir, out);
            }
            return;
        }
        let Some(hit) = world else {
            if show {
                out.push(event(from, from + dir * left, None, None));
            }
            return;
        };
        let point = from + dir * hit.distance;
        let body = w.body(hit.entity);
        if show {
            out.push(event(from, point, Some(body), Some(hit.normal)));
        }
        // Something that takes damage (a map's breakable): the round's
        // damage, banded and dampened as a character's, no zone.
        let u = rng.next();
        let d = base_damage(dmg, u, false, point.distance(r.origin)) * dampen;
        // Its push (`IMPULSE_PER_HIT_POINT`): on a physics object, and as
        // the damage's force (what wakes a map's props).
        let push = dir * d * HIT_POINTS * IMPULSE_PER_HIT_POINT;
        if walk.lands && w.bodies.get(body).is_ok_and(|(rb, _)| rb.is_dynamic()) {
            w.impulses.write(crate::weapon::impact::BulletImpulse {
                body,
                point,
                impulse: push,
            });
        }
        if walk.lands && (w.ground.2.contains(hit.entity) || w.ground.2.contains(body)) {
            w.damage.write(Damage {
                target: if w.ground.2.contains(body) { body } else { hit.entity },
                attacker: Some(r.owner),
                amount: d,
                point,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bullet,
                weapon: None,
                force: push,
            });
        }
        // Through it? A collider's surface, else the world's there.
        let name = w.surface_name(hit.entity, point);
        let Some(pass) = w.surface_at(hit.entity, point) else { return };
        let restart = if pass.thickness <= 0.0 {
            point
        } else {
            // Cast back from the limit + 1 unit beyond the entry to 1 unit
            // before it: anything it meets passes (the thickness itself is
            // never compared).
            let reach = pass.thickness + unit;
            // The level's own geometry (no surface of its own: the
            // world's) is solid behind its polygons, which are open
            // meshes here: a point inside a thick wall is found by looking
            // for the wall's far side ahead, not by the back-cast alone
            // (the original's cast from inside solid space hits at once).
            let ahead = Dir3::new(dir).unwrap_or(Dir3::NEG_Z);
            if !w.owns_surface(hit.entity)
                && w.spatial.cast_ray(point + dir * 1e-3, ahead, reach, false, &filter).is_none()
            {
                return;
            }
            let far = point + dir * reach;
            let back = Dir3::new(-dir).unwrap_or(Dir3::NEG_Z);
            let Some(h) = w.spatial.cast_ray(far, back, reach + unit, true, &filter) else { return };
            if h.distance <= 0.0 {
                // Still inside it at the limit: too thick.
                return;
            }
            // An object (not the world or a world model: effects_sounds.md
            // Follow-up (2026-10-31b) 3) must be thinner than the limit on
            // some axis too.
            if let Ok((rb, aabb)) = w.bodies.get(body)
                && !rb.is_static()
                && !w.ground.3.contains(body)
                && aabb.is_some_and(|b| (b.max - b.min).min_element() >= pass.thickness)
            {
                return;
            }
            // The exit when its surface is the entry's, else the entry.
            let exit = far - dir * h.distance;
            if w.surface_name(h.entity, exit) == name { exit } else { point }
        };
        left = (walk.range - restart.distance(r.origin)) * pass.range;
        dampen *= pass.damage;
        let p = pass.perturb.round() as i32;
        let (k_r, k_u) = (whole_draw(rng.next(), p), whole_draw(rng.next(), p));
        dir = reperturb(dir, k_r, k_u);
        from = restart + dir * 1e-3;
    }
}

/// Damage `d` (health units, before the victim) to the sphere `si` of
/// `b`: the attachments' victim factor, the AI factor, armour and zone,
/// the AI cap, the death threshold.
#[allow(clippy::too_many_arguments)]
pub(super) fn hurt(
    w: &mut Delivery,
    dmg: &CaDamage,
    r: &Round,
    b: &Body,
    si: usize,
    d: f32,
    point: Vec3,
    dir: Vec3,
    out: &mut Vec<WeaponEvent>,
) {
    // The shape hit: its region (its hitgroup, armour location) and zone
    // factor (the body's own, Combat Arms' spheres'; else by region).
    let shape = b.shapes.get().0[si];
    let location = location_of(shape.region);
    let factor = shape.scale.unwrap_or_else(|| zone_factor(shape.region));
    // Hit points from here on (armour points are).
    let hp = 0.01;
    let hitgroup = shape.region.hitgroup();
    let mut damage = Damage {
        target: b.entity,
        attacker: Some(r.owner),
        amount: 0.0,
        point,
        dir,
        hitgroup,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    };
    let lands = w.rules.lands(&damage);
    let infected = w.infected.contains(b.entity);
    // The attachments' special functions (attachments.md, "Bullet slot"):
    // the victim-side factor before armour, by target group and
    // probability (each function's own draw).
    let npc = w.attach.0.get(b.entity).ok().copied();
    let victim = Victim {
        npc: npc.is_some(),
        zombie: infected || npc.is_some_and(|n| n.zombie),
    };
    let forced = w.attach.1.0;
    let mut draws = Rng::new(r.seed.wrapping_mul(0x9e37_79b9).wrapping_add(si as u32));
    let mut draw = |_: usize| forced.unwrap_or_else(|| draws.next());
    // The shooter's gear first (Fighting Chance, then the sets: hit
    // points, attacker side before armour).
    let d = match (w.dealt.get(r.owner), w.ids.get(r.weapon), &w.gear_data) {
        (Ok(fx), Ok(weapon), Some(data)) => fx.dealt(&data.0, weapon.id, d / hp) * hp,
        _ => d,
    };
    // Then an AI's damage x `AIDamageFactor` (Corrections 7).
    let d = d * dmg.mods.victim_factor(victim, &mut draw) * w.ai.factor(Some(r.owner));
    let enemy = !w.same_team(r.owner, b.entity);
    // Stun: the shooter's passing Stun functions slow the victim (movement's
    // P_stun) for their duration.
    if lands
        && enemy
        && let Some((penalty, duration)) = dmg.mods.stun(SpecialKind::Stun, victim, &mut draw)
    {
        w.commands.entity(b.entity).insert(crate::game::movement::StunPenalty {
            penalty,
            until: w.now.now + duration as f64,
        });
    }
    let knock = dmg.mods.stun(SpecialKind::Knockback, victim, &mut draw).unwrap_or_default();
    let ff = w.friendly_scale(r.owner, b.entity);
    let hit = ArmourHit {
        damage_type: dmg.damage_type,
        location,
        zone: if w.mercs.contains(r.owner) { 1.0 } else { factor },
        infected,
    };
    let after = match w.armor.get_mut(b.entity) {
        Ok((a, pieces)) => armour_and_zone(d * ff / hp, hit, Some(a.into_inner()), pieces, lands),
        Err(_) => armour_and_zone(d * ff / hp, hit, None, None, lands),
    };
    // An AI's hit on a player is capped (not melee).
    let points = w.ai.cap(Some(r.owner), b.entity, after, dmg.damage_type == MELEE) * hp;
    // An infected victim hit by the other team is knocked back (movement.md,
    // "Hit knock-back").
    if infected && lands && enemy {
        let stiff = w.figures.get(b.entity).ok().flatten().map_or(crate::game::movement::movement_data().character.figure.stiff_duration, |c| c.figure.stiff_duration);
        let current = w.knockback.get(b.entity).ok();
        if let Some(k) =
            crate::game::movement::HitKnockback::hit(current, dmg.move_penalty + knock.0, dmg.move_penalty_duration + knock.1, stiff, dir, w.now.now)
        {
            w.commands.entity(b.entity).insert(k);
        }
    }
    let health = w.characters.get(b.entity).map_or(0.0, |(.., h, _, _, _)| h.current);
    // Dead at 1 HP or less.
    damage.amount = if points > 0.0 && health - points <= DEATH_THRESHOLD * hp + 1e-6 {
        points.max(health)
    } else {
        points
    };
    out.push(WeaponEvent {
        owner: r.owner,
        weapon: r.weapon,
        kind: WeaponEventKind::Hit {
            target: b.entity,
            amount: damage.amount,
            hitgroup,
        },
        replay: r.replay,
    });
    // The rules' friendly-fire step scales the message again (S8): it
    // carries the amount before that step.
    damage.amount /= ff;
    if let Some(regions) = w.regions.as_mut() {
        regions.write(crate::core::RegionHit {
            target: b.entity,
            region: shape.region,
        });
    }
    w.damage.write(damage);
}

