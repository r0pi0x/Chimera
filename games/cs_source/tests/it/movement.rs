//! CS:S's movement on the greybox: its ladder and water tank work with
//! Source movement (the engine's own movement on it: chimera_sim's
//! tests/it/movement.rs).

use bevy::prelude::*;

use crate::stack::{
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
};

/// The greybox's ladder and water tank work with Source movement, so they
/// can be tried by hand (`--movement cs_source:movement`).
#[test]
fn greybox_ladder_and_tank_with_source_movement() {
    use crate::stack::games::cs_source::movement::{self as source, SourceMovement, SourceMovementPlugin};
    let state = |sim: &Sim, p: Entity| sim.app.world().get::<SourceMovement>(p).unwrap().clone();

    // In front of the north-wall ladder, facing it (-Z is intent yaw 0).
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    let p = sim.spawn_character(
        Vec3::new(greybox::LADDER_X, 1.0, greybox::NORTH_WALL_Z + 0.1 + 0.43),
        source::ID,
    );
    sim.seconds(0.3);
    sim.intent(p).move_axis = Vec2::Y;
    let y0 = sim.position(p).y;
    sim.seconds(1.0);
    assert!(state(&sim, p).ladder.is_some(), "on the ladder");
    assert!(sim.position(p).y - y0 > 3.0, "climbed {} m", sim.position(p).y - y0);

    // Dropped into the tank: under water, sinking slowly.
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    let c = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    let p = sim.spawn_character(Vec3::new(c.x, 1.3, c.y), source::ID);
    sim.seconds(0.3);
    assert_eq!(state(&sim, p).water_level, 3, "head under water");
    let vy = sim.velocity(p).y;
    assert!(vy < 0.0 && vy > -1.3, "sinking at {vy} m/s (48 units/s at most)");
}

/// A bot added while the local player flies with noclip moves with the
/// match's movement, not the player's current one (it took the player's
/// slot, so `bot_add` in noclip made a bot that flew through walls): it
/// falls onto the floor like any walking character.
#[test]
fn a_bot_added_while_the_player_flies_falls_onto_the_floor() {
    use crate::stack::{
        bot::{BotConfig, add_bot},
        core::{LocalPlayer, Team},
        games::cs_source::{
            movement::{self as source, SourceMovementPlugin},
            weapons::CsWeaponsPlugin,
        },
        movement::noclip,
        slots::{Loadout, MovementSlot},
    };
    for loadout in [false, true] {
        let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin, SourceMovementPlugin));
        if loadout {
            sim.app.insert_resource(Loadout { movement: source::ID });
        }
        sim.app.world_mut().resource_mut::<BotConfig>().stop = 1;
        let me = sim.spawn_character(greybox::SPAWNS[0] + Vec3::Y * 3.0, noclip::ID);
        sim.app.world_mut().entity_mut(me).insert((Team(2), LocalPlayer));
        sim.ticks(1);
        let bot = add_bot(sim.app.world_mut(), Team(1)).expect("a bot");
        sim.ticks(1);
        let slot = |sim: &Sim| sim.app.world().get::<MovementSlot>(bot).map(|m| m.0);
        assert_eq!(slot(&sim), Some(source::ID), "loadout {loadout}");
        // Lifted 2 m, it falls back onto the floor.
        let floor = sim.position(bot).y;
        sim.app.world_mut().get_mut::<Transform>(bot).unwrap().translation.y += 2.0;
        if let Some(mut p) = sim.app.world_mut().get_mut::<avian3d::prelude::Position>(bot) {
            p.0.y += 2.0;
        }
        sim.seconds(1.5);
        assert!(sim.state(bot).on_ground, "loadout {loadout}: landed");
        assert!((sim.position(bot).y - floor).abs() < 0.2, "loadout {loadout}: on the floor");
        // The player is still flying: only the bot changed.
        assert_eq!(sim.app.world().get::<MovementSlot>(me).map(|m| m.0), Some(noclip::ID));
    }
}

/// CS:S's golden run (docs/plans/active/shared-movement.md, section 7):
/// a scripted course on the greybox with a brush staircase added (a run
/// and a sprint, strafe jumps with a second press in the air, a duck
/// jump, the stairs, a fall with damage, the ladder and a jump off it, a
/// swim and a water jump out of the tank, a crouch walk), every
/// tick's place, velocity, flags, eye, view roll, health and stamina
/// hashed. The hash is the one taken before the movement's code was moved
/// (the plan's step S0): a refactor that changes one float operation
/// changes it. A behaviour change re-takes it in its own commit, saying
/// why. The run prints its work counts (the step's instructions and the
/// whole tick's, per tick), which the plan's log keeps per step.
/// `CHIMERA_GOLDEN_TRACE=1` prints every tick's row, to find the first
/// tick two builds differ at. Asserted on Linux x86-64 only (elsewhere the
/// maths library's last bits may differ: printed, not asserted). The same
/// course runs in cs_source's, hl2dm's, tf2's and dods' tests/it/movement.rs.
#[test]
fn golden_run() {
    use crate::stack::{
        core::{Health, MapBrush, Velocity},
        games::cs_source::movement::{self as source, SourceMovement, SourceMovementPlugin},
        harness::own_process,
        map::MapBrushes,
    };
    const GOLDEN: u64 = 0x2c28df5ccf0d4a27;
    const TICKS: u64 = 1080;
    if !own_process("movement::golden_run") {
        return;
    }
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    sim.repeatable();
    let p = sim.spawn_character(greybox::SPAWNS[0], source::ID);
    sim.ticks(2);
    // A brush staircase east of the spawn, rising toward -Z: 8 steps of
    // 12 units (0.3048 m) by 24, swept exactly (the greybox's own solids
    // are physics colliders).
    const STAIRS: Vec3 = Vec3::new(30.0, 0.0, 10.0);
    let step = 12.0 * 0.0254;
    let mut brushes = sim.app.world().resource::<MapBrushes>().0.clone();
    for i in 0..8 {
        let z = STAIRS.z - 2.0 * step * i as f32;
        brushes.push(MapBrush::from_box(
            Vec3::new(STAIRS.x - 1.5, 0.0, z - if i == 7 { 4.0 } else { 2.0 * step }),
            Vec3::new(STAIRS.x + 1.5, step * (i + 1) as f32, z),
        ));
    }
    sim.app.world_mut().insert_resource(MapBrushes(brushes));

    let place = |sim: &mut Sim, at: Vec3| {
        let w = sim.app.world_mut();
        w.get_mut::<Transform>(p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    };
    let tank = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    let trace = std::env::var_os("CHIMERA_GOLDEN_TRACE").is_some();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut add = |bits: u32| {
        for b in bits.to_le_bytes() {
            hash = (hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let (mut step_work, mut tick_work, mut counted) = (0u64, 0u64, true);
    // What the course met: the air, a duck, a ladder, a head under water,
    // the stairs' top, fall damage.
    let mut met = [false; 6];
    for t in 0..TICKS {
        {
            let mut i = sim.intent(p);
            i.move_axis = Vec2::ZERO;
            (i.jump, i.crouch, i.sprint, i.walk, i.use_key) = (false, false, false, false, false);
            i.pitch = 0.0;
            match t {
                // Settle, then a run north (-Z), its last part sprinting.
                0..30 => {}
                30..130 => {
                    i.move_axis = Vec2::Y;
                    i.sprint = t >= 100;
                }
                // Strafe jumps: a jump every 40 ticks and a second press
                // in the air, strafing right while turning right.
                130..290 => {
                    i.move_axis = Vec2::new(1.0, if (t - 130) % 40 < 4 { 1.0 } else { 0.0 });
                    i.jump = matches!((t - 130) % 40, 0 | 14);
                    i.yaw -= 1.5f32.to_radians();
                }
                // A duck jump, then standing up.
                290..350 => {
                    i.move_axis = Vec2::Y;
                    i.jump = t == 292;
                    i.crouch = (296..330).contains(&t);
                }
                // Up the stairs and off their top.
                350..500 => i.move_axis = Vec2::Y,
                // A fall from 9 m, then a walk with the walk key held.
                500..620 => {
                    i.move_axis = Vec2::new(0.0, if t >= 590 { 1.0 } else { 0.0 });
                    i.walk = true;
                }
                // Up the north wall's ladder (a use press on the way, for
                // the movements that mount by it), a jump off it, away and
                // back to the wall.
                620..800 => {
                    i.move_axis = Vec2::new(0.0, if t < 650 || t >= 740 { 1.0 } else { -1.0 });
                    i.pitch = if t < 650 { 0.6 } else { 0.0 };
                    i.use_key = t == 622;
                    i.jump = t == 650;
                }
                // The tank: sinking, then a swim up and north and out over
                // its wall.
                800..1020 => {
                    if t >= 860 {
                        i.move_axis = Vec2::Y;
                        i.jump = true;
                        i.pitch = 0.3;
                    }
                }
                // A crouch walk, let go.
                _ => {
                    i.move_axis = Vec2::new(-1.0, 1.0);
                    i.crouch = t < 1060;
                }
            }
        }
        match t {
            290 | 350 | 500 | 620 | 800 | 1020 => sim.intent(p).yaw = 0.0,
            _ => {}
        }
        match t {
            350 => place(&mut sim, STAIRS + Vec3::new(0.0, 1.0, 3.0)),
            500 => place(&mut sim, Vec3::new(20.0, 10.0, 20.0)),
            620 => place(&mut sim, Vec3::new(greybox::LADDER_X, 1.0, greybox::NORTH_WALL_Z + 0.1 + 0.43)),
            800 => place(&mut sim, Vec3::new(tank.x, 4.5, tank.y)),
            1020 => place(&mut sim, greybox::SPAWNS[1]),
            _ => {}
        }
        match sim.work(1) {
            Some(w) => {
                tick_work += w.total();
                step_work += w.crate_systems.get("source_engine Movement step").map_or(0, |v| v.0);
            }
            None => counted = false,
        }
        let (at, v, s) = (sim.position(p), sim.velocity(p), sim.state(p));
        let me = sim.app.world().get::<SourceMovement>(p).unwrap().clone();
        let health = sim.app.world().get::<Health>(p).map_or(0.0, |h| h.current);
        for f in [at.x, at.y, at.z, v.x, v.y, v.z, s.eye_offset.y, s.view_roll, health, me.stamina] {
            add(f.to_bits());
        }
        add(u32::from(s.on_ground)
            | u32::from(s.crouching) << 1
            | u32::from(s.on_ladder) << 2
            | u32::from(me.ducked) << 3
            | u32::from(me.ducking) << 4
            | u32::from(me.water_level) << 8);
        if trace {
            eprintln!(
                "{t} at {at:?} v {v:?} eye {} roll {} hp {health} stamina {} ground {} crouch {} ladder {} ducked {} ducking {} water {}",
                s.eye_offset.y, s.view_roll, me.stamina, s.on_ground, s.crouching, s.on_ladder, me.ducked, me.ducking, me.water_level
            );
        }
        met[0] |= !s.on_ground;
        met[1] |= me.ducked;
        met[2] |= s.on_ladder;
        met[3] |= me.water_level == 3;
        met[4] |= (350..500).contains(&t) && at.y > 2.5;
        met[5] |= health < 1.0;
    }
    eprintln!("cs_source golden run: hash {hash:#018x}, met (air, duck, ladder, swim, stairs, fall damage) {met:?}");
    if counted {
        eprintln!(
            "cs_source golden run work: the step {} instructions a tick, the tick {} ({TICKS} ticks)",
            step_work / TICKS,
            tick_work / TICKS
        );
    } else {
        eprintln!("cs_source golden run work: no instruction counters here, not counted");
    }
    assert_eq!(met, [true; 6], "the course met (air, duck, ladder, swim, stairs, fall damage)");
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(hash, GOLDEN, "the golden run's hash changed: {hash:#018x} (docs/plans/active/shared-movement.md 7)");
    } else {
        eprintln!("cs_source golden run: the hash is asserted on Linux x86-64 only");
    }
}
