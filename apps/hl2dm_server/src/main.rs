//! The HL2:DM dedicated server's binary (`hl2dm_server`): the library's
//! `ServerPlugin` on a headless run loop, the console on stdin (`status`,
//! `changelevel`, `quit`, any cvar), its output on stdout. Options:
//! `--help` (lib.rs's `USAGE`).
//!
//! At most 300 frames a second (`FPS_MAX`, as `cs_source_server`),
//! schedules on one thread each (`single_threaded_schedules`;
//! `CHIMERA_EXECUTOR=multi` keeps Bevy's). Released built alone: `cargo
//! build --release -p hl2dm_server` (no presentation links).

use std::{
    io::BufRead,
    sync::{Mutex, mpsc},
    time::Duration,
};

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use chimera_core::console::{Console, Level};
use hl2dm_server::{Parsed, ServerPlugin, StartMap, USAGE};

/// Frames a second at most: a packet waits at most a frame (3.3 ms).
const FPS_MAX: f64 = 300.0;

fn main() {
    let options = match hl2dm_server::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(o)) => o,
        Ok(Parsed::Help) => {
            println!("{USAGE}");
            return;
        }
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    let start = StartMap::load(&options).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    if let StartMap::Loaded { id, data, .. } = &start {
        eprintln!("loaded {id}: {} spawns, {} warnings", data.spawns.len(), data.warnings.len());
    }
    let mut app = App::new();
    chimera_world::harness::add_headless_with(
        &mut app,
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / FPS_MAX))),
    );
    // Ctrl+C exits as `quit` does (AppExit: `net::udp` hangs up on every
    // client).
    app.add_plugins((bevy::log::LogPlugin::default(), bevy::app::TerminalCtrlCHandlerPlugin))
        .add_plugins(ServerPlugin::new(start, &options))
        .add_systems(Update, (read_stdin, start_serving, print_console).chain());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    app.insert_resource(Stdin(Mutex::new(rx)));
    if chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi") {
        chimera_world::harness::single_threaded_schedules(&mut app);
    }
    app.run();
}

/// Lines typed on stdin.
#[derive(Resource)]
struct Stdin(Mutex<mpsc::Receiver<String>>);

fn read_stdin(stdin: Res<Stdin>, mut console: ResMut<Console>) {
    let rx = stdin.0.lock().unwrap();
    while let Ok(line) = rx.try_recv() {
        console.print(Level::Input, format!("] {line}"));
        console.submit(line);
    }
}

/// Listen once the command line's console lines have run (they set the
/// port and player count).
fn start_serving(world: &mut World, mut started: Local<bool>) {
    if *started {
        return;
    }
    *started = true;
    if let Err(e) = hl2dm_server::listen(world) {
        eprintln!("error: {e}");
        world.write_message(AppExit::error());
    }
}

/// Console output (command results, errors) to stdout.
fn print_console(console: Res<Console>, mut printed: Local<u64>) {
    let new = console.printed.saturating_sub(*printed) as usize;
    *printed = console.printed;
    let lines = &console.output;
    for line in &lines[lines.len().saturating_sub(new)..] {
        if line.level != Level::Input {
            println!("{}", line.text);
        }
    }
}
