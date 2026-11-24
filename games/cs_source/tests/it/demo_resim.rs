//! Recorded input played again (`games::cs_source::resim`, the `demosim`
//! tool) on a client demo built in code: its user commands written as a
//! client's `dem_usercmd` frames (`source_formats::dem::writer`), read
//! back through the demo reader and CS:S's mapper, with the recording
//! player's own-client states (tick base ahead of the command numbers by
//! a fixed amount, as a server's) and shots from a run of our own, driven
//! here without the harness under test. Played again it must match tick
//! for tick (the offset found, seeds and accuracy equal), a state changed
//! by a unit is found at its command, and the server's settings
//! (`net_SetConVar`) are applied. Synthetic recordings of a gun's state
//! at the start: a silenced USP (its attack2 toggle replayed), a FAMAS in
//! burst mode, an XM1014 mid-recovery and mid-cycle. The probe's real
//! CS:S runs: `heavy::demo_resim`.

use bevy::{ecs::message::Messages, prelude::*};
use source_formats::dem::{
    Header, ServerInfo,
    usercmd::{UserCmd, buttons},
    writer::{DemoWriter, Message},
};

use crate::stack::{
    core::{Seed, SimClock, SimTick},
    games::cs_source::{
        TICK_INTERVAL,
        demo::{self, CsDemo, FL_DUCKING, FL_ONGROUND, Happening, LocalSnap, PlayerSnap, Thing},
        movement::{self as source, QuakeMovement, SourceMovementPlugin, to_source},
        resim::{self, Options, Recording},
        weapons::{CsWeaponsPlugin, Inaccuracy},
    },
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
    weapon::{AltModes, Inventory, Magazine, SpreadShape, WeaponEvent, WeaponEventKind, WeaponState, give},
};

/// Tick base minus command number (a server's count runs ahead of the
/// client's).
const OFFSET: i64 = 5000;
/// The first command's number.
const FIRST: u32 = 1000;
const AK: u16 = 1;

fn make() -> Sim {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// Walk, jump, duck sideways, tap and hold fire while turning, reload.
fn commands() -> Vec<UserCmd> {
    (0..140u32)
        .map(|i| {
            let mut c = UserCmd {
                command_number: FIRST + i,
                tick_count: 10_000 + i,
                view_angles: [5.0, 90.0, 0.0],
                ..Default::default()
            };
            let mut press = |b: u32| c.buttons |= b;
            match i {
                10..60 => {
                    press(buttons::FORWARD);
                    if i == 40 {
                        press(buttons::JUMP);
                    }
                }
                60..75 => press(buttons::DUCK | buttons::MOVERIGHT),
                80 | 85 => press(buttons::ATTACK),
                95..110 => press(buttons::ATTACK | buttons::FORWARD),
                120 => press(buttons::RELOAD),
                _ => {}
            }
            if c.buttons & buttons::FORWARD != 0 {
                c.forward_move = 400.0;
            }
            if c.buttons & buttons::MOVERIGHT != 0 {
                c.side_move = 400.0;
            }
            if (95..110).contains(&i) {
                c.view_angles[1] = 90.0 + (i - 95) as f32;
            }
            c
        })
        .collect()
}

/// A shot of ours: the command's place, seed, inaccuracy and spread.
type Shot = (usize, u32, f32, f32);

/// Our own run of `cmds` (with `maxspeed`), driven here: the player's
/// state before the first command, after each, and the shots.
fn reference(cmds: &[UserCmd], maxspeed: f32) -> (Vec<PlayerSnap>, Vec<Shot>) {
    let mut sim = make();
    let mut config = source::config();
    config.maxspeed = maxspeed;
    source::set_config(sim.app.world_mut(), config);
    let p = sim.spawn_character(greybox::SPAWNS[0], source::ID);
    sim.ticks(1);
    give(sim.app.world_mut(), p, "weapon_ak47").unwrap();
    sim.seconds(1.5);
    let snap = |sim: &Sim| -> PlayerSnap {
        let w = sim.app.world();
        let me = w.get::<QuakeMovement>(p).unwrap();
        let a = w.get::<Inventory>(p).unwrap().active.unwrap();
        let m = w.get::<Magazine>(a).unwrap();
        let origin = (to_source(sim.position(p)) - Vec3::Z * 36.0).to_array();
        PlayerSnap {
            origin,
            pitch: 5.0,
            yaw: 90.0,
            team: 3,
            class: 5,
            health: 100,
            alive: true,
            flags: if me.on_ground { FL_ONGROUND } else { 0 } | if me.ducked { FL_DUCKING } else { 0 },
            weapon: Some(AK),
            local: Some(LocalSnap {
                origin,
                velocity: to_source(sim.velocity(p)).to_array(),
                ducked: me.ducked,
                ducking: false,
                tick_base: 0,
                shots_fired: w.get::<WeaponState>(a).unwrap().burst as i32,
                clip: Some(m.clip as i32),
                reserve: Some(m.reserve as i32),
                weapons: vec![AK],
            }),
            ..Default::default()
        }
    };
    let mut states = vec![snap(&sim)];
    let mut shots = Vec::new();
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor();
    for (i, c) in cmds.iter().enumerate() {
        let next = sim.app.world().resource::<SimTick>().0 + 1;
        sim.app
            .world_mut()
            .entity_mut(p)
            .insert(Seed(c.command_number.wrapping_sub(next as u32) as u64));
        {
            let mut it = sim.intent(p);
            it.move_axis = Vec2::new(c.side_move / 400.0, c.forward_move / 400.0);
            it.yaw = c.view_angles[1].to_radians() - std::f32::consts::FRAC_PI_2;
            it.pitch = -c.view_angles[0].to_radians();
            it.jump = c.buttons & buttons::JUMP != 0;
            it.crouch = c.buttons & buttons::DUCK != 0;
            it.fire = c.buttons & buttons::ATTACK != 0;
            it.reload = c.buttons & buttons::RELOAD != 0;
        }
        sim.ticks(1);
        assert_eq!(sim.intent(p).command, c.command_number, "the command's number stamped");
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if let WeaponEventKind::Fired {
                seed,
                spread: SpreadShape::Disc { inaccuracy, spread },
                ..
            } = e.kind
            {
                shots.push((i, seed, inaccuracy, spread));
            }
        }
        states.push(snap(&sim));
    }
    (states, shots)
}

/// The client demo: a signon packet (server info: the recorder in slot 0,
/// so entity 1; the server's `sv_maxspeed`), then per command a packet
/// and its `dem_usercmd` frame; read back, then the recorder's states
/// added to the frames (the state before command `i` in command `i`'s
/// frame, keyed by tick base) with its shots in the frame after them.
fn client_demo(cmds: &[UserCmd], states: &[PlayerSnap], shots: &[Shot], maxspeed: f32) -> CsDemo {
    let header = Header {
        demo_protocol: 3,
        network_protocol: 24,
        server_name: "loopback".into(),
        client_name: "Recorder".into(),
        map: "greybox".into(),
        game_dir: "cstrike".into(),
        playback_time: 0.0,
        ticks: 0,
        frames: 0,
        signon_length: 0,
    };
    let mut w = DemoWriter::new(&header);
    w.packet(
        0,
        true,
        &[
            Message::ServerInfo(ServerInfo {
                protocol: 24,
                player_slot: 0,
                max_clients: 2,
                tick_interval: TICK_INTERVAL as f32,
                os: 'L',
                game_dir: "cstrike".into(),
                map: "greybox".into(),
                host_name: "test".into(),
                ..Default::default()
            }),
            Message::SetConVar(vec![
                ("sv_maxspeed".into(), maxspeed.to_string()),
                ("mp_friendlyfire".into(), "0".into()),
            ]),
        ],
    );
    for (i, c) in cmds.iter().enumerate() {
        let tick = i as i32 + 1;
        w.packet(tick, false, &[Message::Tick(20_000 + tick as u32)]);
        w.usercmd(tick, 7 + i as i32, c);
    }
    let mut demo = demo::read(&w.finish()).expect("the written demo reads");
    assert!(demo.timeline.errors.is_empty(), "{:?}", demo.timeline.errors);
    demo.classes = vec!["CWorld".into(), "CAK47".into()];
    assert_eq!(demo.recorder, Some(1));
    assert_eq!(demo.timeline.frames.len(), cmds.len());
    for (i, f) in demo.timeline.frames.iter_mut().enumerate() {
        assert!(
            matches!(f.events.as_slice(), [Happening::Command { cmd, .. }] if cmd.command_number == FIRST + i as u32),
            "command {i} read back"
        );
        let mut s = states[i].clone();
        let tick_base = (FIRST as i64 + i as i64 - 1) + OFFSET;
        s.local.as_mut().unwrap().tick_base = tick_base;
        f.entities = vec![(1, Thing::Player(s.clone()))];
        for &(_, seed, inaccuracy, spread) in shots.iter().filter(|(at, ..)| *at + 1 == i) {
            f.events.push(Happening::Shot {
                player: 1,
                origin: s.origin,
                pitch: 5.0,
                yaw: 90.0,
                mode: 0,
                seed,
                inaccuracy,
                spread,
            });
        }
    }
    demo
}

fn run(rec: &Recording) -> resim::Report {
    let report = resim::resimulate(&make, rec, Options::default()).expect("re-simulated");
    println!("{report}");
    report
}

#[test]
fn a_client_demo_replays_tick_for_tick() {
    let cmds = commands();
    let (states, shots) = reference(&cmds, 320.0);
    assert!(shots.len() >= 4, "the run fired: {shots:?}");
    let demo = client_demo(&cmds, &states, &shots, 320.0);
    let rec = Recording::from_demo(&demo, None).unwrap();
    assert_eq!(rec.commands.len(), cmds.len());
    assert_eq!(rec.states.len(), cmds.len());
    assert_eq!(rec.commands[40].buttons, buttons::FORWARD | buttons::JUMP);
    assert!(rec.states.iter().all(|s| s.alt_mode == Some(0)), "no silencer or burst");
    let r = run(&rec);
    assert_eq!(r.offset, OFFSET, "the tick base's lead found");
    // The last state is in no frame (its frame would be the next command's).
    assert_eq!(r.compared, cmds.len() - 1);
    assert!(r.first.is_none(), "{:?}", r.first);
    assert!(r.max_position < 0.01, "{}", r.max_position);
    assert_eq!(
        r.shots.recorded,
        shots.len() - usize::from(shots.iter().any(|s| s.0 + 1 == cmds.len()))
    );
    assert_eq!(r.shots.same_tick, r.shots.recorded);
    assert_eq!(r.shots.seeds_equal, r.shots.recorded, "seeds from the command numbers");
    assert!(r.shots.max_inaccuracy_diff < 1e-5, "{}", r.shots.max_inaccuracy_diff);
    assert!(r.cvars.contains(&("sv_maxspeed".into(), "320".into())), "{:?}", r.cvars);
}

#[test]
fn a_difference_shows_at_its_command() {
    let cmds = commands();
    let (states, shots) = reference(&cmds, 320.0);
    let mut demo = client_demo(&cmds, &states, &shots, 320.0);
    // The state after command 50 (in command 51's frame) one unit off.
    let f = &mut demo.timeline.frames[51];
    let Thing::Player(p) = &mut f.entities[0].1 else {
        unreachable!()
    };
    p.local.as_mut().unwrap().origin[0] += 1.0;
    // Command 100's clip one round more.
    let f = &mut demo.timeline.frames[101];
    let Thing::Player(p) = &mut f.entities[0].1 else {
        unreachable!()
    };
    *p.local.as_mut().unwrap().clip.as_mut().unwrap() += 1;
    let r = run(&Recording::from_demo(&demo, None).unwrap());
    let d = r.first.expect("a divergence");
    assert_eq!((d.index, d.key), (50, FIRST as i64 + 50));
    assert!(d.what.starts_with("position 1.0"), "{}", d.what);
    assert_eq!(r.clip, 1, "the clip once");
    assert!((r.max_position - 1.0).abs() < 0.01);
}

#[test]
fn the_servers_settings_are_applied() {
    // Recorded on a server with sv_maxspeed 200 (the AK-47's own is 221).
    let cmds = commands();
    let (states, shots) = reference(&cmds, 200.0);
    let demo = client_demo(&cmds, &states, &shots, 200.0);
    let mut rec = Recording::from_demo(&demo, None).unwrap();
    let r = run(&rec);
    assert!(r.first.is_none(), "{:?}", r.first);
    // Without them, our default runs faster as soon as it walks.
    rec.cvars.clear();
    rec.offset = Some(OFFSET);
    let r = run(&rec);
    let d = r.first.expect("a divergence without the settings");
    assert!((10..20).contains(&d.index), "{}: {}", d.index, d.what);
}

// ---------------------------------------------------------------------------
// A gun's state at the start: a synthetic recording (the probe's
// convention: command t leads to state t + 1) of a run of ours with the
// weapon's alternate state (silencer, burst), accuracy penalty and timers
// as they were.

/// `count` commands, each with the buttons whose range holds its place.
fn gun_commands(press: &[(u32, std::ops::Range<u32>)], count: u32) -> Vec<UserCmd> {
    (0..count)
        .map(|i| UserCmd {
            command_number: FIRST + i,
            tick_count: 10_000 + i,
            view_angles: [5.0, 90.0, 0.0],
            buttons: press
                .iter()
                .filter(|(_, at)| at.contains(&i))
                .fold(0, |b, (k, _)| b | k),
            ..Default::default()
        })
        .collect()
}

/// Our run of `cmds` with `weapon` in alternate mode `mode`: the state
/// before the first command and after each (the weapon's alternate mode,
/// penalty and timers included), and the shots.
fn gun_run(weapon: &str, mode: u8, cmds: &[UserCmd]) -> (Vec<resim::Observed>, Vec<resim::RecordedShot>) {
    let mut sim = make();
    let p = sim.spawn_character(greybox::SPAWNS[0], source::ID);
    sim.ticks(1);
    let w = give(sim.app.world_mut(), p, weapon).unwrap();
    sim.seconds(1.5);
    if let Some(mut m) = sim.app.world_mut().get_mut::<AltModes>(w) {
        m.current = mode;
    }
    let snap = |sim: &Sim, key: i64| -> resim::Observed {
        let world = sim.app.world();
        let me = world.get::<QuakeMovement>(p).unwrap();
        let st = world.get::<WeaponState>(w).unwrap();
        let m = world.get::<Magazine>(w).unwrap();
        // The next command runs at the next tick's time.
        let next = world.resource::<SimClock>().now + TICK_INTERVAL;
        let wait = |t: f64| (t - next) as f32;
        resim::Observed {
            key,
            tick: key as i32,
            origin: (to_source(sim.position(p)) - Vec3::Z * 36.0).to_array(),
            velocity: Some(to_source(sim.velocity(p)).to_array()),
            on_ground: me.on_ground,
            ducked: me.ducked,
            weapon: Some(weapon.into()),
            clip: Some(m.clip as i32),
            reserve: Some(m.reserve as i32),
            shots_fired: Some(st.burst as i32),
            inaccuracy: world.get::<Inaccuracy>(w).map(|i| i.value),
            alt_mode: world.get::<AltModes>(w).map(|m| m.current),
            waits: Some([
                wait(st.next_primary),
                wait(st.next_secondary),
                wait(world.get::<Inventory>(p).unwrap().next_attack),
            ]),
            carried: Vec::new(),
        }
    };
    let mut states = vec![snap(&sim, 0)];
    let mut shots = Vec::new();
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor();
    for (i, c) in cmds.iter().enumerate() {
        let next = sim.app.world().resource::<SimTick>().0 + 1;
        sim.app
            .world_mut()
            .entity_mut(p)
            .insert(Seed(c.command_number.wrapping_sub(next as u32) as u64));
        {
            let mut it = sim.intent(p);
            it.yaw = c.view_angles[1].to_radians() - std::f32::consts::FRAC_PI_2;
            it.pitch = -c.view_angles[0].to_radians();
            it.fire = c.buttons & buttons::ATTACK != 0;
            it.secondary = c.buttons & buttons::ATTACK2 != 0;
        }
        sim.ticks(1);
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if let WeaponEventKind::Fired {
                seed,
                spread: SpreadShape::Disc { inaccuracy, spread },
                ..
            } = e.kind
            {
                shots.push(resim::RecordedShot {
                    key: i as i64,
                    seed: Some(seed),
                    inaccuracy: Some(inaccuracy),
                    spread: Some(spread),
                    ..Default::default()
                });
            }
        }
        states.push(snap(&sim, i as i64 + 1));
    }
    (states, shots)
}

/// The recording of `cmds` from state `from` on.
fn synthetic(cmds: &[UserCmd], states: &[resim::Observed], shots: &[resim::RecordedShot], from: usize) -> Recording {
    Recording {
        name: "synthetic".into(),
        map: "greybox".into(),
        tick_interval: TICK_INTERVAL as f32,
        commands: cmds
            .iter()
            .enumerate()
            .skip(from)
            .map(|(i, c)| resim::Command {
                key: i as i64,
                tick: i as i32,
                number: c.command_number,
                buttons: c.buttons,
                pitch: c.view_angles[0],
                yaw: c.view_angles[1],
                ..Default::default()
            })
            .collect(),
        states: states[from..].to_vec(),
        shots: shots.iter().filter(|s| s.key >= from as i64).cloned().collect(),
        offset: Some(1),
        ..Default::default()
    }
}

fn assert_tick_for_tick(r: &resim::Report, shots: usize) {
    assert!(r.first.is_none(), "{:?}", r.first);
    assert_eq!(
        (r.shots.recorded, r.shots.ours),
        (shots, shots),
        "{:?}",
        r.shots.first_mismatch
    );
    assert_eq!(r.shots.same_tick, shots);
    assert_eq!(r.shots.seeds_equal, shots);
    assert!(r.shots.max_inaccuracy_diff < 1e-6, "{}", r.shots.max_inaccuracy_diff);
    assert!(r.max_inaccuracy < 1e-6, "{}", r.max_inaccuracy);
}

#[test]
fn a_silenced_usp_starts_silenced_and_its_toggle_replays() {
    // Taps with the silencer on, attack2 takes it off (3 s), taps again.
    let taps = [10..11, 30..31, 50..51, 290..291, 310..311];
    let mut press: Vec<(u32, std::ops::Range<u32>)> = taps.iter().map(|t| (buttons::ATTACK, t.clone())).collect();
    press.push((buttons::ATTACK2, 70..71));
    let cmds = gun_commands(&press, 330);
    let (states, shots) = gun_run("weapon_usp", 1, &cmds);
    assert_eq!(states[0].alt_mode, Some(1));
    assert_eq!(states.last().unwrap().alt_mode, Some(0), "attack2 took it off");
    assert_eq!(shots.len(), taps.len());
    let r = run(&synthetic(&cmds, &states, &shots, 0));
    assert_tick_for_tick(&r, shots.len());
    assert_eq!(
        (r.alt_mode, r.alt_mode_applied),
        (0, 0),
        "the toggle replayed as attack2"
    );

    // Without the state recorded ours starts unsilenced: each shot adds
    // the unsilenced USP's penalty.
    let mut rec = synthetic(&cmds, &states, &shots, 0);
    rec.states.iter_mut().for_each(|s| s.alt_mode = None);
    let r = run(&rec);
    let d = r.first.expect("the penalty differs");
    assert!(d.what.contains("accuracy"), "{}", d.what);
    assert!(r.max_inaccuracy > 0.005, "{}", r.max_inaccuracy);

    // Placed unsilenced, the recording's silencer (on before the first
    // command, no attack2) is set at the first state that shows it.
    let mut rec = synthetic(&cmds, &states, &shots, 0);
    rec.states[0].alt_mode = Some(0);
    let r = run(&rec);
    assert_tick_for_tick(&r, shots.len());
    assert_eq!((r.alt_mode, r.alt_mode_applied), (0, 1));

    // Without the attack2, the recorded change is applied, not a divergence.
    let mut rec = synthetic(&cmds, &states, &shots, 0);
    rec.commands.iter_mut().for_each(|c| c.buttons &= !buttons::ATTACK2);
    let r = run(&rec);
    assert_eq!((r.alt_mode, r.alt_mode_applied), (0, 1));
    assert_eq!(r.shots.same_tick, shots.len());
}

#[test]
fn a_famas_starts_in_burst_mode() {
    let cmds = gun_commands(&[(buttons::ATTACK, 10..11), (buttons::ATTACK, 60..61)], 90);
    let (states, shots) = gun_run("weapon_famas", 1, &cmds);
    assert_eq!(shots.len(), 6, "two bursts of three");
    let r = run(&synthetic(&cmds, &states, &shots, 0));
    assert_tick_for_tick(&r, shots.len());

    // Started in single shots (the state not recorded): one a pull.
    let mut rec = synthetic(&cmds, &states, &shots, 0);
    rec.states.iter_mut().for_each(|s| s.alt_mode = None);
    let r = run(&rec);
    assert_eq!(r.shots.ours, 2);
    assert!(r.first.is_some());
}

#[test]
fn a_replay_starts_mid_recovery_and_mid_cycle() {
    // An XM1014 shot, then the recording starts 6 commands later: penalty
    // still recovering, the next shot not yet allowed; fire held through.
    let cmds = gun_commands(&[(buttons::ATTACK, 5..6), (buttons::ATTACK, 12..40)], 60);
    let (states, shots) = gun_run("weapon_xm1014", 0, &cmds);
    let from = 11;
    assert!(states[from].waits.unwrap()[0] > 0.05, "{:?}", states[from].waits);
    assert!(states[from].inaccuracy.unwrap() > 0.02, "{:?}", states[from].inaccuracy);
    let rec = synthetic(&cmds, &states, &shots, from);
    assert!(!rec.shots.is_empty());
    let r = run(&rec);
    assert_tick_for_tick(&r, rec.shots.len());
}
