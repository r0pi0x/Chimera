//! A Halo player's death as its body shows it (specs/halo_ce/
//! health_shields.md "Death", "Stun and pings"; animations.md "Damage and
//! death", "Playback"; multiplayer_rules.md 5.4): the body left where the
//! player fell, playing the death the killing hit picked, on every
//! machine.
//!
//! **The choice** (`choose`, on the server at the death, `leave_corpses`):
//! a unit in the air (not on the ground) plays its seat's airborne death
//! (`stand airborne-dead`); otherwise the damage table's kill slot
//! (`anims::damage_key`) for the hit's direction and body part, hard
//! (`h-kill`) when the killing hit was a hard ping (forced by a headshot or
//! a shield it broke: `vitality::HaloPing`), else soft (`s-kill`); a
//! hard kill on the head uses the left set. The direction is the
//! killing ping's, else the hit's travel against the unit's facing by
//! the spec's cones (`vitality::ping_direction`; a hit with no direction
//! counts as from the front); the part is the hit's (`part_of`: the
//! engine's hitgroup, which keeps the arm and the leg but not the hand or
//! the foot apart: an arm's hit plays the arm's set, a leg's the foot's,
//! the parts the Spartan's nodes give legs; no hitgroup is the gut, the
//! spec's "a hit without a node").
//!
//! **The body** (`HaloCorpse`): a separate entity the server spawns at the
//! dead player's feet with its facing, armour colour, velocity and death
//! animation, replicated to every client (net part `halo_ce_corpses`); the
//! player entity itself is hidden while dead (the engine's `Dead`) and
//! respawns elsewhere, leaving the body. Every machine plays it the same
//! way (`play_corpses`, once per Halo tick): the animation starts at frame
//! 0 blended in over 3 ticks from the player's last pose there
//! (`UNIT_DAMAGE_TRANSITION_TICKS`), advances one frame a tick and holds its
//! last frame; its sound (the animation's sound reference, at its sound
//! frame: "Playback") plays at the body on that machine; a body that died
//! in the air falls under Halo's gravity until a ray down meets the level
//! (`CorpseState::fall`), then lies there, playing its landing death. It
//! is drawn as the engine's
//! `map::Corpse` (Halo's game identity picks the Spartan, `BodySkin` its
//! colour, the pose its own `Animator::fixed`); it has no collider and
//! no hit volumes. The server removes a body lying idle (its animation
//! ended, on the ground) for more than 900 ticks (`CORPSE_LIFE`), and
//! every body at a map change.
//!
//! Characters only: a body that is no Halo body (`HaloBody`: another
//! game's character in a party app) leaves nothing here; a Halo body
//! killed by any game's hit does (no ping: a soft kill).
//!
//! Known shortcuts:
//! - No limp body physics (animations.md: "a biped then starts its limp
//!   body" once its animation ends): the last frame holds.
//! - An airborne death that lands plays the seat's landing death
//!   (`landing-dead`, blended in over 3 ticks): inferred from the slot and
//!   its body-fall sound, the spec's states do not say when it plays.
//! - One permutation per slot (the chain's first).
//! - A machine plays the death from when it hears of the body (a client
//!   some ticks after the server; one joining later sees it from the
//!   start).
//! - The body is no object: shots pass through it (Halo's dead bipeds
//!   still take hits and show their blood, effects.md "corpse").

use bevy::prelude::*;
use blam_formats::animation::{BodyPart, DamageDirection, DamageKind, UnitDamageSlot};
use serde::{Deserialize, Serialize};

use super::{
    HaloBody,
    anims::{self, AIRBORNE_DEAD, Blend, LANDING_DEAD, SEATS, base_pose, damage_key, frames, seat_death_key, sound_of},
};
use super::super::{GAME, METERS_PER_UNIT, vitality::{HaloPing, PingDirection, Spartan, ping_direction}};
use crate::{
    core::{Died, Hitgroup, Intent, MapLoads, MovementState, SimClock, Velocity},
    map::{BodySkin, CharacterModels, Corpse, PlaySound, anim::{AnimSet, Animator, BonePose, nlerp}},
};

/// Ticks of the blend into a death (animations.md, UNIT_DAMAGE_TRANSITION_TICKS).
pub const UNIT_DAMAGE_TRANSITION_TICKS: u32 = 3;

/// Ticks a dead biped lies idle before it is deleted (multiplayer_rules.md
/// CORPSE_LIFE: deleted when idle for more than this).
pub const CORPSE_LIFE: u32 = 900;

/// A dead Halo player's body (module doc): replicated, the server's.
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HaloCorpse {
    /// The player who died (its last pose starts the blend).
    #[entities]
    pub owner: Entity,
    /// Its armour colour (`body::COLOURS`).
    pub skin: i32,
    /// Its feet at the death (engine space, metres).
    pub at: Vec3,
    /// Its facing (as `Intent::yaw`).
    pub yaw: f32,
    /// Its velocity at the death (m/s): an airborne body keeps falling.
    pub velocity: Vec3,
    /// The death's sequence (`choose`); empty: none, the bind pose.
    pub animation: String,
}

/// What picks a death (`choose`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeathInputs {
    /// The unit is in the air.
    pub airborne: bool,
    /// The killing hit was a hard ping.
    pub hard: bool,
    pub direction: PingDirection,
    pub part: BodyPart,
}

/// The damage table's direction of a ping's.
pub fn table_direction(d: PingDirection) -> DamageDirection {
    match d {
        PingDirection::Front => DamageDirection::Front,
        PingDirection::Left => DamageDirection::Left,
        PingDirection::Right => DamageDirection::Right,
        PingDirection::Back => DamageDirection::Back,
    }
}

/// The body part of a hit by the engine's hitgroup (module doc).
pub fn part_of(h: Hitgroup) -> BodyPart {
    match h {
        Hitgroup::Generic | Hitgroup::Stomach => BodyPart::Gut,
        Hitgroup::Chest => BodyPart::Chest,
        Hitgroup::Head => BodyPart::Head,
        Hitgroup::LeftArm => BodyPart::LeftArm,
        Hitgroup::RightArm => BodyPart::RightArm,
        Hitgroup::LeftLeg => BodyPart::LeftFoot,
        Hitgroup::RightLeg => BodyPart::RightFoot,
    }
}

/// The damage table's kill slot for `i` (module doc; `airborne` aside).
pub fn kill_slot(i: &DeathInputs) -> UnitDamageSlot {
    let kind = if i.hard { DamageKind::HardKill } else { DamageKind::SoftKill };
    let direction = if i.hard && i.part == BodyPart::Head { DamageDirection::Left } else { table_direction(i.direction) };
    UnitDamageSlot::new(kind, direction, i.part)
}

/// The death's sequence name (module doc): None when `set` has no such
/// slot (the state is entered with no animation).
pub fn choose(set: &AnimSet, i: &DeathInputs) -> Option<String> {
    let name = if i.airborne { seat_death_key(SEATS[0], AIRBORNE_DEAD) } else { damage_key(kill_slot(i)) };
    set.names.contains_key(&name).then_some(name)
}

/// The direction of a hit travelling along `dir` (engine space) on a unit
/// of yaw `yaw`: the spec's cones, the front for no direction.
pub fn direction_of(spartan: &Spartan, dir: Vec3, yaw: f32) -> PingDirection {
    let flat = Vec2::new(dir.x, dir.z);
    if !flat.is_finite() || flat.length_squared() < 1e-12 {
        return PingDirection::Front;
    }
    ping_direction(spartan, dir, Quat::from_rotation_y(yaw) * Vec3::NEG_Z)
}

/// A body's play on this machine (module doc), not replicated.
#[derive(Component, Clone, Debug, Default)]
pub struct CorpseState {
    /// The death's sequence in the set (None: none).
    pub sequence: Option<usize>,
    pub frame: usize,
    /// The first tick is shown (frame 0, no advance).
    pub started: bool,
    /// Feet (metres) and velocity (metres per Halo tick).
    pub at: Vec3,
    pub velocity: Vec3,
    /// On the ground.
    pub resting: bool,
    /// Halo ticks lying idle (its animation over, on the ground).
    pub idle: u32,
    pub blend: Option<Blend>,
    /// An airborne death's landing death, played when it lands (None:
    /// none, or played).
    pub landing: Option<usize>,
    /// It landed this tick.
    pub landed: bool,
    /// The last pose made (a landing's blend starts from it).
    pub last: Vec<BonePose>,
}

impl CorpseState {
    /// The body's start: at its feet, falling with its velocity unless
    /// `grounded`; `from` the player's last pose here (the blend).
    pub fn new(set: &AnimSet, corpse: &HaloCorpse, grounded: bool, from: Option<Vec<BonePose>>) -> Self {
        Self {
            sequence: set.names.get(&corpse.animation).copied(),
            at: corpse.at,
            velocity: if grounded { Vec3::ZERO } else { corpse.velocity / 30.0 },
            resting: grounded,
            landing: (corpse.animation == seat_death_key(SEATS[0], AIRBORNE_DEAD))
                .then(|| set.names.get(&seat_death_key(SEATS[0], LANDING_DEAD)).copied())
                .flatten(),
            blend: from.filter(|f| !f.is_empty()).map(|from| Blend { from, ticks: UNIT_DAMAGE_TRANSITION_TICKS, index: 0 }),
            ..default()
        }
    }

    /// One Halo tick of the fall: gravity `g` (m per tick per tick), then
    /// the move, stopped by the ground `ground(from, length)` finds below
    /// (the distance down to it).
    pub fn fall(&mut self, g: f32, ground: impl Fn(Vec3, f32) -> Option<f32>) {
        if self.resting {
            return;
        }
        self.velocity.y -= g;
        let drop = (-self.velocity.y).max(0.0);
        if let Some(d) = ground(self.at, drop)
            && d <= drop
        {
            self.at.y -= d;
            self.at.x += self.velocity.x;
            self.at.z += self.velocity.z;
            self.velocity = Vec3::ZERO;
            self.resting = true;
            self.landed = true;
            return;
        }
        self.at += self.velocity;
    }

    /// One Halo tick of the animation ("Playback": the sound at its frame
    /// before the advance, then the frame, held at the last): the pose,
    /// and the sound to play now.
    pub fn tick(&mut self, set: &AnimSet) -> (Vec<BonePose>, Option<String>) {
        let mut sound = None;
        // An airborne death landing: its landing death, blended in.
        if std::mem::take(&mut self.landed)
            && let Some(l) = self.landing.take()
        {
            self.sequence = Some(l);
            self.frame = 0;
            self.started = false;
            self.blend = (!self.last.is_empty())
                .then(|| Blend { from: self.last.clone(), ticks: UNIT_DAMAGE_TRANSITION_TICKS, index: 0 });
        }
        let ended = if let Some(s) = self.sequence {
            let n = frames(set, s);
            if self.started {
                if let Some((f, name)) = sound_of(set, s)
                    && f == self.frame
                {
                    sound = Some(name.to_string());
                }
                self.frame = (self.frame + 1).min(n - 1);
            }
            self.frame + 1 >= n
        } else {
            true
        };
        self.started = true;
        let mut pose = match self.sequence {
            Some(s) => base_pose(set, set.sequences[s].anims[0], self.frame),
            None => set.defaults.clone(),
        };
        if let Some(b) = &mut self.blend {
            let u = (b.index + 1) as f32 / b.ticks as f32;
            for ((q, p), (fq, fp)) in pose.iter_mut().zip(&b.from) {
                *q = nlerp(*fq, *q, u);
                *p = fp.lerp(*p, u);
            }
            b.index += 1;
            if b.index >= b.ticks {
                self.blend = None;
            }
        }
        if ended && self.resting {
            self.idle += 1;
        }
        self.last.clone_from(&pose);
        (pose, sound)
    }

    /// Lying idle for more than `CORPSE_LIFE` ticks.
    pub fn expired(&self) -> bool {
        self.idle > CORPSE_LIFE
    }
}

/// Halo's body set of the map's models.
fn halo_set(models: &CharacterModels) -> Option<std::sync::Arc<AnimSet>> {
    models.0.iter().find(|m| m.game == Some(GAME)).and_then(|m| m.animations.clone())
}

/// The server: each Halo body that died this tick leaves its corpse
/// (module doc).
#[allow(clippy::type_complexity)]
pub fn leave_corpses(
    mut died: MessageReader<Died>,
    mut pings: MessageReader<HaloPing>,
    models: Option<Res<CharacterModels>>,
    spartan: Res<Spartan>,
    players: Query<
        (&Transform, &Intent, &MovementState, Option<&Velocity>, Option<&BodySkin>, Option<&avian3d::prelude::ColliderAabb>),
        With<HaloBody>,
    >,
    mut commands: Commands,
) {
    let kills: Vec<HaloPing> = pings.read().filter(|p| p.kill).copied().collect();
    let deaths: Vec<Died> = died.read().cloned().collect();
    let Some(set) = models.as_deref().and_then(halo_set) else { return };
    for d in deaths {
        let Ok((at, intent, state, velocity, skin, aabb)) = players.get(d.entity) else { continue };
        let ping = kills.iter().rev().find(|p| p.target == d.entity);
        let inputs = DeathInputs {
            airborne: !state.on_ground,
            hard: ping.is_some_and(|p| p.hard),
            direction: ping.map_or_else(|| direction_of(&spartan, d.damage.dir, intent.yaw), |p| p.direction),
            part: part_of(d.damage.hitgroup),
        };
        // Its feet: the collider's bottom (as the engine draws bodies).
        let feet = at.translation.with_y(aabb.map_or(at.translation.y + state.hull_min.y, |b| b.min.y));
        commands.spawn((
            Name::new("halo_ce corpse"),
            bevy_replicon::prelude::Replicated,
            Transform::from_translation(feet).with_rotation(Quat::from_rotation_y(intent.yaw)),
            HaloCorpse {
                owner: d.entity,
                skin: skin.map_or(0, |s| s.0),
                at: feet,
                yaw: intent.yaw,
                velocity: velocity.map_or(Vec3::ZERO, |v| v.0),
                animation: choose(&set, &inputs).unwrap_or_default(),
            },
        ));
    }
}

/// Every machine: each body's play this tick (module doc), drawn as the
/// engine's corpse.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn play_corpses(
    models: Option<Res<CharacterModels>>,
    clock: Res<SimClock>,
    spatial: avian3d::prelude::SpatialQuery,
    characters: Query<(), With<Intent>>,
    owners: Query<&HaloBody>,
    mut corpses: Query<(Entity, &HaloCorpse, Option<&mut CorpseState>, Option<&mut Animator>, Option<&mut Transform>)>,
    mut sounds: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    let Some(set) = models.as_deref().and_then(halo_set) else { return };
    let ticks = super::super::ticks::halo_ticks(&clock).min(4);
    let g = super::super::movement::MovementData::builtin().gravity * METERS_PER_UNIT;
    let filter = avian3d::prelude::SpatialQueryFilter::from_mask(crate::core::SOLID_LAYERS);
    let ground = |from: Vec3, drop: f32| {
        let lift = 0.05;
        spatial
            .cast_ray_predicate(from + Vec3::Y * lift, Dir3::NEG_Y, drop + lift, true, &filter, &|e| !characters.contains(e))
            .map(|h| h.distance - lift)
    };
    for (e, corpse, state, animator, transform) in &mut corpses {
        let Some(mut state) = state else {
            // Its first tick here: drawn as a corpse from now on.
            let from = owners.get(corpse.owner).ok().map(|b| b.anim.last.clone());
            let grounded = corpse.velocity.y <= 0.0 && ground(corpse.at, 0.1).is_some();
            let mut state = CorpseState::new(&set, corpse, grounded, from);
            let (pose, _) = state.tick(&set);
            let mut animator = Animator::new(set.clone());
            animator.fixed = Some(chimera_core::later::Later::ready(pose));
            animator.main = Some(0);
            commands.entity(e).insert((
                state,
                animator,
                Corpse { feet: 0.0 },
                BodySkin(corpse.skin),
                chimera_core::slots::GameIdentity(GAME.into()),
                Visibility::default(),
                Transform::from_translation(corpse.at).with_rotation(Quat::from_rotation_y(corpse.yaw)),
            ));
            continue;
        };
        let Some(mut animator) = animator else { continue };
        for _ in 0..ticks {
            state.fall(g, ground);
            let (pose, sound) = state.tick(&set);
            animator.fixed = Some(chimera_core::later::Later::ready(pose));
            if let Some(entry) = sound {
                sounds.write(PlaySound::at(entry, state.at + Vec3::Y * 0.5));
            }
        }
        let want = Transform::from_translation(state.at).with_rotation(Quat::from_rotation_y(corpse.yaw));
        if let Some(mut t) = transform
            && *t != want
        {
            *t = want;
        }
    }
}

/// The server: bodies lying idle too long, and every body at a map
/// change, removed.
pub fn remove_corpses(
    loads: Option<Res<MapLoads>>,
    mut seen: Local<Option<u32>>,
    corpses: Query<(Entity, Option<&CorpseState>), With<HaloCorpse>>,
    mut commands: Commands,
) {
    let load = loads.map(|l| l.0);
    let changed = seen.is_some() && *seen != load;
    *seen = load;
    for (e, state) in &corpses {
        if changed || state.is_some_and(CorpseState::expired) {
            commands.entity(e).despawn();
        }
    }
}

/// The corpses' replication (net part `halo_ce_corpses`).
pub fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    app.replicate::<HaloCorpse>();
}

/// The frame count of `set`'s sequence `name` (1 without it): the held
/// weapon's drop delay is a share of it (health_shields.md "Death").
pub fn death_frames(set: &AnimSet, name: &str) -> usize {
    set.names.get(name).map_or(1, |s| frames(set, *s))
}

/// The sound of `name`'s sequence and its frame, for tests and tools.
pub fn death_sound<'a>(set: &'a AnimSet, name: &str) -> Option<(usize, &'a str)> {
    set.names.get(name).and_then(|s| anims::sound_of(set, *s))
}

/// Dev console commands for screenshots of deaths (`--features dev`):
/// `halo_ce:dummy <metres ahead> [turn, degrees: 0 faces you] [metres up]`
/// stands a Halo character (the nearest other one, else a new one) in
/// front of the local player; `halo_ce:kill_dummies <front|back|left|right>
/// [hitgroup]` kills every other living character with a hit from that
/// side of it.
#[cfg(feature = "dev")]
pub fn dev_commands(app: &mut App) {
    use chimera_core::console::ConsoleAppExt;
    use crate::core::{Damage, DamageKind, Health, LocalPlayer, Team};
    app.console_command(
        "halo_ce:dummy",
        "dummy <metres ahead> [turn degrees, 0: facing you] [metres up]: stand a Halo character in front of you (screenshots)",
        |w, a| {
            let num = |i: usize, d: f32| a.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(d);
            let (ahead, turn, up) = (num(0, 4.0), num(1, 0.0).to_radians(), num(2, 0.0));
            let mut q = w.query_filtered::<(&Transform, &Intent), With<LocalPlayer>>();
            let (me, look) = q.iter(w).next().map(|(t, i)| (t.translation, i.yaw)).ok_or("no local player")?;
            let at = me + Quat::from_rotation_y(look) * Vec3::NEG_Z * ahead + Vec3::Y * up;
            let yaw = look + std::f32::consts::PI + turn;
            let mut others = w.query_filtered::<(Entity, &Transform), (With<Intent>, Without<LocalPlayer>)>();
            let near = others.iter(w).min_by(|x, y| x.1.translation.distance(me).total_cmp(&y.1.translation.distance(me))).map(|(e, _)| e);
            let e = match near {
                Some(e) => e,
                None => {
                    let e = w.spawn(chimera_core::character::character_bundle(Transform::from_translation(at), Team(0))).id();
                    chimera_core::slots::set_movement(e, super::super::movement::ID).apply(w);
                    e
                }
            };
            if let Some(mut t) = w.get_mut::<Transform>(e) {
                t.translation = at;
            }
            if let Some(mut p) = w.get_mut::<avian3d::prelude::Position>(e) {
                p.0 = at;
            }
            if let Some(mut i) = w.get_mut::<Intent>(e) {
                i.yaw = yaw;
            }
            Ok(Some(format!("dummy {e} at {at}")))
        },
    );
    app.console_command(
        "halo_ce:kill_dummies",
        "kill_dummies <front|back|left|right> [head|chest|stomach|...]: kill every other living character by a hit from that side (screenshots)",
        |w, a| {
            let side = match a.first().map(String::as_str) {
                Some("back") => std::f32::consts::PI,
                Some("left") => std::f32::consts::FRAC_PI_2,
                Some("right") => -std::f32::consts::FRAC_PI_2,
                _ => 0.0,
            };
            let hitgroup = match a.get(1).map(String::as_str) {
                Some("head") => Hitgroup::Head,
                Some("stomach") => Hitgroup::Stomach,
                Some("leftarm") => Hitgroup::LeftArm,
                Some("rightarm") => Hitgroup::RightArm,
                Some("leftleg") => Hitgroup::LeftLeg,
                Some("rightleg") => Hitgroup::RightLeg,
                _ => Hitgroup::Chest,
            };
            let mut q = w.query_filtered::<(Entity, &Transform, &Intent, &Health), Without<LocalPlayer>>();
            let hits: Vec<Damage> = q
                .iter(w)
                .filter(|(.., h)| h.current > 0.0)
                .map(|(e, t, i, _)| Damage {
                    target: e,
                    attacker: None,
                    amount: 50.0,
                    point: t.translation,
                    // From that side of it: travelling into it.
                    dir: -(Quat::from_rotation_y(i.yaw + side) * Vec3::NEG_Z),
                    hitgroup,
                    kind: DamageKind::Bullet,
                    weapon: None,
                    force: Vec3::ZERO,
                })
                .collect();
            let n = hits.len();
            for d in hits {
                w.write_message(d);
            }
            Ok(Some(format!("{n} killed")))
        },
    );
}
