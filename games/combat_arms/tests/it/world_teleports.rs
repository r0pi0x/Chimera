//! specs/combat_arms/world_objects.md Follow-up (2026-10-31b) 2.3 on the
//! object world: a teleport (`GOTO`, `INIT`) still carries what stands on
//! the object on the server (case 47: a non-player rider, here a bot),
//! while a player, whose place is their own client's, isn't carried by a
//! teleport (2.2, case 46: the client takes the new pose and never carries
//! its player on it); a frame's move carries everyone.

use bevy::prelude::*;
use combat_arms::game::objects::{
    commands::Who,
    movers::CharBox,
    runtime::{Fx, keys},
};

use crate::world_objects::{T, close, obj, player, v, wm};

/// A keyframer `KF` moving the solid platform `Plat` (100 x 10 x 100 u,
/// its top at y 5) from key 0 at the origin to key 1 300 u up in 1 s.
fn lift() -> T {
    let mut b = Vec::new();
    b.extend(2u32.to_le_bytes());
    for (y, stamp) in [(0.0f32, 0.0f32), (300.0, 1.0)] {
        b.extend(0u16.to_le_bytes());
        b.extend([0u8, 0u8]);
        for f in [0.0, y, 0.0, 0.0, 0.0, 0.0, stamp, 0.0] {
            b.extend(f.to_le_bytes());
        }
    }
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let mut t = T::new(vec![
        obj(
            "KeyFramer",
            "KF",
            &[("ObjectName", "Plat".into()), ("BaseKeyName", "K".into()), (keys::KEYS, hex)],
        ),
        wm("WorldModel", "Plat", Vec3::ZERO, Vec3::new(50.0, 5.0, 50.0), &[]),
    ]);
    t.frame();
    t
}

/// A character `id` standing on a top at height `top`; a bot unless
/// `is_player`.
fn rider(id: u64, top: f32, is_player: bool) -> CharBox {
    let mut c = player(id, Vec3::new(-10.0, top, -10.0), Vec3::new(10.0, top + 100.0, 10.0));
    c.player = is_player;
    c
}

fn carried(t: &T, id: u64) -> Vec3 {
    t.w.fx
        .iter()
        .filter_map(|f| match f {
            Fx::Carry { who, delta } if *who == Who::Character(id) => Some(*delta),
            _ => None,
        })
        .sum()
}

/// 47. `GOTO` the key 300 u up: the bot standing on the platform is moved
/// 300 u up in the same frame; the player standing on it too isn't.
#[test]
fn wo47_goto_carries_riders_on_the_server() {
    let mut t = lift();
    t.w.chars = vec![rider(1, 5.0, false), rider(2, 5.0, true)];
    t.w.fx.clear();
    t.send("KF", "GOTO K01");
    close(t.pos("Plat").y, 300.0, 1e-3, "jumped");
    let bot = carried(&t, 1);
    assert!(bot.abs_diff_eq(Vec3::new(0.0, 300.0, 0.0), 1e-3), "the bot carried: {bot}");
    assert_eq!(carried(&t, 2), Vec3::ZERO, "a player isn't carried by a teleport");
}

/// A frame's move (not a teleport) carries the player too.
#[test]
fn a_frames_move_carries_every_rider() {
    let mut t = lift();
    t.send("KF", "ON");
    t.frame();
    let top = t.pos("Plat").y + 5.0;
    t.w.chars = vec![rider(2, top, true)];
    t.w.fx.clear();
    t.frame();
    assert!(carried(&t, 2).y > 0.0, "carried by the move");
}

/// 46 on the server: `INIT` puts a lifted sliding world model back at once
/// (a teleport): a bot standing on it goes with it, the player standing on
/// it doesn't (its client keeps it where it is).
#[test]
fn wo46_init_teleports_without_the_player() {
    let mut t = T::new(vec![wm(
        "SlidingWorldModel",
        "Lift",
        Vec3::ZERO,
        Vec3::new(50.0, 5.0, 50.0),
        &[("MoveDir", v(0.0, 1.0, 0.0)), ("MoveDist", "100".into()), ("PowerOnTime", "1".into())],
    )]);
    t.send("Lift", "TRIGGER");
    t.frames(25);
    close(t.pos("Lift").y, 100.0, 1e-3, "up");
    t.w.chars = vec![rider(1, 105.0, false), rider(2, 105.0, true)];
    t.w.fx.clear();
    t.send("Lift", "INIT");
    close(t.pos("Lift").y, 0.0, 1e-3, "back at once");
    assert!(carried(&t, 1).abs_diff_eq(Vec3::new(0.0, -100.0, 0.0), 1e-3), "the bot");
    assert_eq!(carried(&t, 2), Vec3::ZERO, "not the player");
}
