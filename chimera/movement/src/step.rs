//! The movement step, one system in `core::Predict::Movement`
//! (`MovementStep`): what every character's tick needs gathered once
//! (other characters' boxes, moving solids, ladder and water volumes, the
//! physics filter, the brush grid), then each character's `Mover` built, run
//! and written back (transform, velocity, `MovementState`, sounds, fall
//! damage, map outputs).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    config::{HALF_WIDTH, MovementConfigs, QuakeConfig},
    core::{
        BaseVelocity, Damage, EntityGravity, ExternalPush, Health, Hitgroup, Intent, LadderZone, MapBrush, MaxSpeed,
        MovementState, MovingSolid, Velocity, WaterZone,
    },
    hooks::{MoveSpeed, MoveStance, MoveTraits, StepSounds},
    map::{
        MapBrushCollider, MapBrushTree, MapBrushes, MapTerrain, MapTerrainCollider, MapWater, PhysicsProp, PlaySound,
        PropSurface, PushAway,
        sound::{SoundBank, SurfaceGrid},
    },
    mover::lines::{self, UseableLadder, UseableLadders},
    mover::{Mover, POST_HALF},
    pushaway, shadow,
    slots::MovementSlot,
    state::{SettleOnGround, QuakeMovement},
    trace::Tracer,
    units::{METERS_PER_UNIT, ORIGIN_ABOVE_FEET, to_engine, to_units},
};

#[inline]
pub(crate) fn hull_height(cfg: &QuakeConfig, ducked: bool) -> f32 {
    if ducked { cfg.duck_height } else { cfg.stand_height }
}

pub(crate) fn step(
    mut q: Query<(
        Entity,
        &Intent,
        &mut QuakeMovement,
        &mut Transform,
        &mut Velocity,
        &mut MovementState,
        Option<&MaxSpeed>,
        Option<&mut BaseVelocity>,
        Option<&EntityGravity>,
        Option<&mut shadow::PhysicsTouch>,
        Option<&crate::core::MapControls>,
        Has<SettleOnGround>,
        (
            Option<&MovementSlot>,
            Option<&MoveSpeed>,
            Option<&StepSounds>,
            Option<&mut crate::core::IgnoreFallDamage>,
            Option<&MoveTraits>,
            Option<&mut ExternalPush>,
            Option<&MoveStance>,
        ),
    ), Without<crate::core::Carried>>,
    query: SpatialQuery,
    brushes: Option<Res<MapBrushes>>,
    water: Option<Res<MapWater>>,
    brush_colliders: Query<Entity, With<MapBrushCollider>>,
    (terrain, terrain_colliders, movers, tree, shadow_props, zones): (
        Option<Res<MapTerrain>>,
        Query<Entity, With<MapTerrainCollider>>,
        Query<(Entity, &MovingSolid)>,
        Option<Res<MapBrushTree>>,
        shadow::ShadowProps,
        (
            Query<&WaterZone>,
            Query<&LadderZone>,
            Query<(Entity, &crate::map::held::Held, Option<&RigidBodyColliders>)>,
        ),
    ),
    // Props that are there: a broken or killed one (body and collider
    // disabled until a round restart) neither blocks nor pushes back.
    props: Query<
        (Entity, &Transform, &PhysicsProp),
        (Without<QuakeMovement>, Without<RigidBodyDisabled>, Without<ColliderDisabled>),
    >,
    surfaces: Option<Res<SurfaceGrid>>,
    prop_surfaces: Query<&PropSurface>,
    bank: Option<Res<SoundBank>>,
    mut play: MessageWriter<PlaySound>,
    mut damage: MessageWriter<Damage>,
    (configs, mut ladders, mut outputs, (published, map_game)): (
        MovementConfigs,
        Option<ResMut<UseableLadders>>,
        MessageWriter<crate::map::entities::FireEntityOutput>,
        (Option<Res<crate::core::MapLadders>>, Option<Res<crate::core::MapGame>>),
    ),
    (clock, first, mut grid_cache, mut waist_steps): (
        Res<crate::core::SimClock>,
        Res<crate::core::FirstTimePredicted>,
        Local<Option<crate::core::BrushGrid>>,
        Local<u32>,
    ),
    other_characters: Query<(Entity, &ColliderAabb, Option<&Health>), (With<Intent>, Without<QuakeMovement>)>,
    (health, dead, noblock, teams, (collider_of, afloat)): (
        Query<&Health>,
        Query<(), With<ColliderDisabled>>,
        Option<Res<crate::core::NoBlock>>,
        Query<&crate::core::Team, Without<crate::core::Npc>>,
        (Query<&ColliderOf>, Query<(), With<crate::map::buoyancy::Floating>>),
    ),
) {
    // `chimera_noblock` (ours): which players pass through which (hostages
    // and other characters without a team always block).
    let noblock = noblock.map_or_else(Default::default, |n| *n);
    let team = |e: Entity| teams.get(e).ok().map(|t| t.0);
    let dt = clock.dt();
    // Other games' volumes: liquids swum in, ladders climbed (swept like
    // movers without an owner).
    let water_zones: Vec<MapBrush> = zones.0.iter().map(|z| z.0.clone()).collect();
    let ladder_zones: Vec<MapBrush> = zones.1.iter().map(|z| MapBrush { ladder: true, ..z.0.clone() }).collect();
    // Another game's ladder lines (HL2's `func_useableladder`s) for a
    // movement climbing contents ladders: thin solid ladder posts along
    // them (2 units across), so its probe finds a ladder face where that
    // game's climber stands (ours: docs/DESIGN.md "Ladders across games").
    let posts: Vec<MapBrush> = published
        .as_ref()
        .map(|p| {
            p.lines
                .iter()
                .map(|l| {
                    let r = Vec3::new(POST_HALF, 0.0, POST_HALF) * METERS_PER_UNIT;
                    let (lo, hi) = (l.bottom.min(l.top) - r, l.bottom.max(l.top) + r);
                    MapBrush { ladder: true, ..MapBrush::from_box(lo, hi) }
                })
                .collect()
        })
        .unwrap_or_default();
    // Every living character's box: Source hulls for Source movers, else
    // the collider's bounds. Those whose collider is off (the dead, also
    // those waiting to respawn with their health, unseen) block nobody.
    let alive = |e: Entity| (health.get(e).is_ok_and(|h| h.current > 0.0) || health.get(e).is_err()) && !dead.contains(e);
    let hull_box = |cfg: &QuakeConfig, feet: Vec3, ducked: bool| {
        let height = if ducked { cfg.duck_height } else { cfg.stand_height };
        let (a, b) = (
            to_engine(feet + Vec3::new(-cfg.half_width, -cfg.half_width, 0.0)),
            to_engine(feet + Vec3::new(cfg.half_width, cfg.half_width, height)),
        );
        MapBrush::from_box(a.min(b), a.max(b))
    };
    // A player being pulled onto or off a ladder isn't solid; a box of its
    // size waits at the end of the pull instead (movement.md "Forced
    // moves").
    let mut boxes: Vec<(Entity, MapBrush)> = q
        .iter()
        .filter(|(e, ..)| alive(*e))
        .map(|(e, _, me, t, .., (slot, .., stance))| {
            let feet = me.forced.map_or(to_units(t.translation) - Vec3::Z * ORIGIN_ABOVE_FEET, |f| f.goal);
            let mut cfg = configs.of(slot).into_owned();
            if let Some(s) = stance {
                s.apply(&mut cfg);
            }
            (e, hull_box(&cfg, feet, me.ducked))
        })
        .collect();
    boxes.extend(
        other_characters
            .iter()
            .filter(|(e, _, h)| h.is_none_or(|h| h.current > 0.0) && !dead.contains(*e))
            .map(|(e, aabb, _)| (e, MapBrush::from_box(aabb.min, aabb.max))),
    );
    // Characters (dead ones too) never block through physics casts.
    let characters: Vec<Entity> = q
        .iter()
        .map(|(e, ..)| e)
        .chain(other_characters.iter().map(|(e, ..)| e))
        .collect();
    // Moving solids (doors, platforms) are swept like brushes, and know
    // their owner (what the player stands on).
    let mover_brushes: Vec<(Entity, &MapBrush)> = movers
        .iter()
        .filter(|(_, m)| m.solid)
        .flat_map(|(e, m)| m.brushes.iter().map(move |b| (e, b)))
        .collect();
    let mover_velocity = |e: Entity| movers.get(e).map_or(Vec3::ZERO, |(_, m)| to_units(m.velocity));
    let floating = |c: Entity| afloat.contains(collider_of.get(c).map_or(c, |o| o.body));
    // With brushes swept exactly, physics queries skip the same brushes.
    // Players pass through multiplayer physics props (their own
    // collision group); props that collide stay in. The same for every
    // player (each is among the characters): built once a tick, not once
    // a player (thousands of brush colliders on a big map).
    let excluded = characters
        .iter()
        .copied()
        .chain(brushes.as_ref().map(|_| brush_colliders.iter()).into_iter().flatten())
        .chain(terrain.as_ref().map(|_| terrain_colliders.iter()).into_iter().flatten())
        .chain(
            props
                .iter()
                .filter(|(_, _, p)| p.push != PushAway::Collide)
                .map(|(e, ..)| e),
        );
    // What a character's body meets (`CHARACTER_FILTER`): players-only
    // surfaces too (another game's map's: Combat Arms' collision
    // blockers), not traces-only ones (Combat Arms' non-solid detail, a
    // ramp of which filled coldseed's doorway) or loose items; a Source
    // map's player clips are among its brushes, their collider left out
    // with the rest.
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(crate::core::CHARACTER_FILTER);
    // The broad phase over the map's brushes, rebuilt with them.
    let grid = &mut *grid_cache;
    if let Some(b) = brushes.as_ref()
        && (b.is_changed() || grid.as_ref().is_none_or(|g| g.len != b.0.len()))
    {
        *grid = Some(crate::core::BrushGrid::new(&b.0));
    }
    let grid = grid.as_ref().filter(|_| brushes.is_some());
    // The map's useable ladders' ends, out of the floors they touch (once).
    if let Some(l) = ladders.as_mut()
        && l.0.iter().any(|l| !l.fixed)
    {
        let tracer = Tracer {
            query: &query,
            filter: &filter,
            brushes: brushes.as_deref(),
            grid,
            tree: tree.as_deref(),
            water: water.as_deref(),
            water_zones: &water_zones,
            surfaces: surfaces.as_deref(),
            prop_surfaces: &|_: Entity| None,
            sounds: None,
            heights: (72.0, 36.0),
            half_width: HALF_WIDTH,
            others: &[],
            terrain: terrain.as_deref(),
            owners: &[],
        };
        lines::fix_ends(&tracer, &mut l.0);
    }
    let ladders: &[UseableLadder] = ladders.as_ref().map_or(&[], |l| &l.0);
    // The loaded map's envelope, each character's body fitted to it.
    let map_envelope = configs.map();
    for (
        entity,
        intent,
        mut me,
        mut transform,
        mut vel,
        mut state,
        weapon_speed,
        mut base,
        gravity,
        touch,
        controls,
        settle,
        (slot, speed, step_sounds, mut ignore_fall, traits, mut pushed, stance),
    ) in &mut q
    {
        // The character's own traits over its movement's config (TF2's
        // classes) and its game's stance's box, the body then fitted to
        // the map (`fitted`).
        let own = configs.own(slot);
        let stance = stance.copied().unwrap_or_default();
        let patched = match (traits, stance.height) {
            (None, None) => None,
            _ => {
                let mut c = traits.map_or_else(|| own.clone(), |t| t.apply(own));
                stance.apply(&mut c);
                Some(c)
            }
        };
        let fitted = patched.as_ref().unwrap_or(own).fitted(map_envelope.as_ref());
        let cfg: &QuakeConfig = &fitted;
        // A character without a movement slot is the app's own game's.
        let foreign = slot.is_some_and(|s| crate::core::foreign_map(map_game.as_deref(), crate::core::movement_game(s.0)));
        // player_speedmod: the movement runs as if this many ticks passed
        // (specs/source/game_entities.md 1, lagged movement); 0 (or less:
        // our choice) holds it still, gravity included.
        let time_scale = controls.map_or(1.0, |c| c.time_scale);
        // A player parented to a map entity is carried by the logic.
        if time_scale <= 0.0 || controls.is_some_and(|c| c.parented) {
            continue;
        }
        let dt = dt * time_scale;
        // The mover stood on isn't part of the saved form (an entity): a
        // network client restores it into `MovementState::ground` after a
        // correction (`net::movers::restore_ground`). Otherwise the two
        // are the same here (written together at the end of each step).
        if me.ground_entity.is_none() && state.ground.is_some() {
            me.ground_entity = state.ground;
        }
        // (Borrowed, not copied: a full server copied every other
        // player's box for every player every tick.)
        let mut others: Vec<&MapBrush> = boxes
            .iter()
            .filter(|(e, _)| *e != entity && noblock.blocks(team(entity), team(*e)))
            .map(|(_, b)| b)
            .collect();
        let mut owners: Vec<Option<Entity>> = vec![None; others.len()];
        for &(e, b) in &mover_brushes {
            others.push(b);
            owners.push(Some(e));
        }
        for b in &ladder_zones {
            others.push(b);
            owners.push(None);
        }
        if cfg.climbs_contents(foreign) && foreign {
            for b in &posts {
                others.push(b);
                owners.push(None);
            }
        }
        let (forward, right) = Mover::view(intent);
        let push_input = pushaway::push_back(
            pushaway::player_box(&transform, &me, cfg),
            forward,
            right,
            props.iter().map(|(_, t, p)| (t, p)),
        );
        let prop_surface = |e: Entity| prop_surfaces.get(e).ok().map(|s| s.0.clone());
        // A holder passes through what it holds (gravity_gun.md 5).
        let mine: Vec<Entity> = crate::map::held::held_by(entity, zones.2.iter()).collect();
        let own_filter = (!mine.is_empty()).then(|| {
            let mut f = filter.clone();
            f.excluded_entities.extend(mine);
            f
        });
        let tracer = Tracer {
            query: &query,
            filter: own_filter.as_ref().unwrap_or(&filter),
            brushes: brushes.as_deref(),
            grid,
            tree: tree.as_deref(),
            water: water.as_deref(),
            water_zones: &water_zones,
            surfaces: surfaces.as_deref(),
            prop_surfaces: &prop_surface,
            sounds: bank.as_ref().map(|b| &*b.0),
            heights: (cfg.stand_height, cfg.duck_height),
            half_width: cfg.half_width,
            others: &others,
            terrain: terrain.as_deref(),
            owners: &owners,
        };
        let mut mover = Mover {
            cfg,
            trace: &tracer,
            feet: to_units(transform.translation) - Vec3::Z * ORIGIN_ABOVE_FEET,
            v: to_units(vel.0),
            me: &mut me,
            dt,
            water_cache: [None; 3],
            push_input,
            sounds: Vec::new(),
            // The held weapon's speed (spec: MaxPlayerSpeed), else the default.
            player_maxspeed: weapon_speed.and_then(|s| s.0).map_or(cfg.player_maxspeed, |s| s / METERS_PER_UNIT),
            fall_damage: 0.0,
            base: base.as_ref().map_or(Vec3::ZERO, |b| to_units(b.velocity)),
            base_touched: base.as_ref().is_some_and(|b| b.touched),
            // A lifting push (`core::ExternalPush`) takes it off as logic does.
            unground: base.as_ref().is_some_and(|b| b.unground) || pushed.as_ref().is_some_and(|p| p.lift),
            push: pushed.as_deref().copied().unwrap_or_default(),
            gravity_scale: gravity.map_or(1.0, |g| g.0),
            mover_velocity: &mover_velocity,
            floating: &floating,
            waist_steps: &mut *waist_steps,
            touched: Vec::new(),
            ground_collider: None,
            wish: Vec3::ZERO,
            settle,
            speed: stance.max_speed.or(traits.and_then(|t| t.max_speed)).or(speed.map(|s| s.0)),
            step_sounds,
            ladders,
            foreign_map: foreign,
            outputs: Vec::new(),
            max_health: health.get(entity).map_or(100.0, |h| h.max * 100.0),
            stance,
        };
        if let Some(on_ground) = mover.me.seat.take() {
            mover.reseat(on_ground);
        }
        let start = mover.feet;
        mover.tick(intent);
        mover.me.use_held = intent.use_key;
        // The velocity modifier recovers after the move, whichever way it
        // went (M18: the first tick after a hit moves at the value set).
        mover.me.velocity_modifier = (mover.me.velocity_modifier + cfg.velocity_modifier_recovery * dt).min(1.0);
        let (feet, v) = (mover.feet, mover.v);
        if let Some(mut touch) = touch {
            *touch = shadow_props.touch(&mover.touched, mover.ground_collider, start, feet, mover.wish, dt);
        }
        if let Some(p) = pushed.as_mut() {
            let new = ExternalPush { lift: false, ..mover.push };
            if **p != new {
                **p = new;
            }
        }
        if let Some(b) = base.as_mut() {
            let new = BaseVelocity {
                velocity: to_engine(mover.base),
                touched: mover.base_touched,
                unground: false,
            };
            if b.velocity != new.velocity || b.touched != new.touched || b.unground {
                **b = new;
            }
        }
        // Damage, sounds and outputs only the first time a command runs.
        if !first.0 {
            mover.sounds.clear();
            mover.outputs.clear();
        }
        for (map_index, output) in mover.outputs.drain(..) {
            outputs.write(crate::map::entities::FireEntityOutput {
                map_index,
                output: output.into(),
                activator: Some(entity),
            });
        }
        // A map's `IgnoreFallDamage` window: no damage (a reset one ends
        // at this landing).
        if mover.fall_damage > 0.0
            && let Some(ignore) = ignore_fall.as_mut()
            && clock.now < ignore.until
        {
            mover.fall_damage = 0.0;
            if ignore.reset && first.0 {
                ignore.until = f64::MIN;
            }
        }
        // TF2's share of max health: times 1 +- the spread, then rounded
        // (specs/tf2/movement.md "Falling and landing"); the spread's draw
        // is the tick's and the character's, the same on every replay.
        if mover.fall_damage > 0.0 && cfg.fall_by_max_health > 0.0 && cfg.fall_fixed.is_none() {
            let r = if cfg.fall_spread > 0.0 {
                let mut x = entity.to_bits() ^ clock.tick.wrapping_mul(0x9e37_79b9_7f4a_7c15);
                x ^= x >> 33;
                x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
                x ^= x >> 33;
                let u = (x >> 11) as f64 / (1u64 << 53) as f64;
                1.0 + cfg.fall_spread * (2.0 * u as f32 - 1.0)
            } else {
                1.0
            };
            mover.fall_damage = (mover.fall_damage * r).round();
        }
        if mover.fall_damage > 0.0 && first.0 {
            // Health is normalized: 1.0 = 100 points. No attacker, no
            // armour (measured: armour doesn't absorb it).
            damage.write(Damage {
                force: bevy::math::Vec3::ZERO,
                target: entity,
                attacker: None,
                amount: mover.fall_damage / 100.0,
                point: to_engine(feet),
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: crate::core::DamageKind::Fall,
                weapon: None,
            });
            // Taking it plays `Player.FallDamage` (`super::pain`).
        }
        for (entry, at, volume) in mover.sounds.drain(..) {
            play.write(PlaySound {
                pitch: None,
                entry,
                at: Some(to_engine(at)),
                volume,
                source: Some(entity),
                channel: Some(cfg.sound_channel),
            });
        }
        transform.translation = to_engine(feet + Vec3::Z * ORIGIN_ABOVE_FEET);
        // Later movers this tick see where this one went.
        if let Some((_, b)) = boxes.iter_mut().find(|(e, _)| *e == entity) {
            *b = hull_box(cfg, me.forced.map_or(feet, |f| f.goal), me.ducked);
        }
        vel.0 = to_engine(v);
        *state = MovementState {
            on_ground: me.on_ground,
            crouching: me.ducked || (me.ducking && intent.crouch),
            sprinting: false,
            eye_offset: Vec3::Y * (me.eye - ORIGIN_ABOVE_FEET) * METERS_PER_UNIT,
            hull_min: {
                let (a, b) = (
                    to_engine(Vec3::new(-cfg.half_width, -cfg.half_width, -ORIGIN_ABOVE_FEET)),
                    to_engine(Vec3::new(cfg.half_width, cfg.half_width, hull_height(cfg, me.ducked) - ORIGIN_ABOVE_FEET)),
                );
                a.min(b)
            },
            hull_max: {
                let (a, b) = (
                    to_engine(Vec3::new(-cfg.half_width, -cfg.half_width, -ORIGIN_ABOVE_FEET)),
                    to_engine(Vec3::new(cfg.half_width, cfg.half_width, hull_height(cfg, me.ducked) - ORIGIN_ABOVE_FEET)),
                );
                a.max(b)
            },
            ground: me.ground_entity,
            on_ladder: me.ladder.is_some() || me.useable.is_some(),
            view_roll: me.punch_roll.to_radians(),
        };
    }
}
