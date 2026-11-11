//! Instructions-per-tick regression gate on CS:S's scenarios
//! (docs/performance.md, "Regression gate"): the gate itself (counters,
//! baselines, verdict) is Chimera's (`chimera_tools::perfgate`); this
//! binary names the scenarios and builds them on the CS:S app's harness.
//!
//! Scenarios: `greybox` (always), `de_dust2` (rounds with 10 bots; needs a
//! CS:S install), `mg_lego_multigames_v2` (a community map, when it is in
//! the content cache). Each runs `--warmup` ticks, then measures `--ticks`
//! ticks one by one by their work counts (`metrics::work`), compared with
//! `tools/perfgate/baseline.json` per build profile (`--update` rewrites
//! it). The scenarios are `Sim::repeatable`: they play out the same on
//! every run.

use bevy::prelude::*;
use chimera_tools::perfgate::{self, Gate};
use cs_source_app::{
    console::Console,
    core::{Intent, Team},
    games::{
        self,
        cs_source::{self, TICK_INTERVAL, movement::SourceMovementPlugin, weapons::CsWeaponsPlugin},
    },
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
    map::MapPlugin,
    mount::config::{LocalConfig, content_dir},
};

const SCENARIOS: [&str; 3] = ["greybox", "de_dust2", "mg_lego_multigames_v2"];

fn main() {
    let gate = Gate {
        name: "perfgate",
        scenarios: &SCENARIOS,
        baseline: cs_source_tools::repo_root().join("tools/perfgate/baseline.json"),
        build: cs_source_app::core::BUILD,
    };
    perfgate::main(gate, |name| Some(Box::new(Run(scenario(name)?))));
}

/// A scenario's simulation, ticked and hashed for the gate.
struct Run(Sim);

impl perfgate::Scenario for Run {
    fn tick(&mut self) {
        self.0.ticks(1);
    }

    fn state_hash(&mut self) -> Option<u64> {
        Some(self.0.state_hash())
    }

    fn state_lines(&mut self) -> Vec<String> {
        self.0.state_rows().into_iter().map(|(e, name, bits)| format!("{e} {name:?} {bits:x?}")).collect()
    }
}

/// The scenario's simulation, ready to tick; None when its map is missing.
fn scenario(name: &str) -> Option<Sim> {
    let mut sim = match name {
        "greybox" => {
            let mut sim = Sim::new(CsSim, GreyboxMapPlugin);
            // Characters walking and turning on fixed patterns (`drive`).
            for (i, at) in greybox::SPAWNS.iter().enumerate() {
                let e = sim.spawn_character(*at, cs_source_app::movement::placeholder::ID);
                sim.app.world_mut().entity_mut(e).insert(Driven(i as u32));
            }
            sim.app.add_systems(FixedPreUpdate, drive);
            sim
        }
        "de_dust2" => {
            if !installed() {
                return None;
            }
            let mut sim = cs_map("de_dust2")?;
            for team in [1, 2] {
                for _ in 0..5 {
                    cs_source_app::bot::add_bot(sim.app.world_mut(), Team(team)).expect("bot");
                }
            }
            sim.app
                .world_mut()
                .resource_mut::<Console>()
                .submit("mp_freezetime 2; mp_roundtime 2; chimera_rounds 1");
            sim
        }
        "mg_lego_multigames_v2" => {
            let cached =
                content_dir(cs_source::GAME).is_some_and(|d| d.join("maps").join(format!("{name}.bsp")).is_file());
            if !installed() || !cached {
                return None;
            }
            cs_map(name)?
        }
        _ => return None,
    };
    // As the game runs them (apps/cs_source/src/main.rs): every per-frame
    // and fixed schedule on one thread, so the counts don't depend on how
    // work was handed out.
    cs_source_app::single_threaded_schedules(&mut sim.app);
    // Tasks' results on the tick they're polled: the same run every time.
    sim.repeatable();
    Some(sim)
}

fn installed() -> bool {
    LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| p.join("cstrike").is_dir())
}

fn cs_map(name: &str) -> Option<Sim> {
    let map = games::load_map(&format!("cs_source:{name}")).ok()?;
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.insert_resource(cs_source_app::slots::Loadout {
        movement: cs_source::movement::ID,
    });
    Some(sim)
}

/// A greybox character's pattern number.
#[derive(Component)]
struct Driven(u32);

/// Walk, strafe, turn and jump on a schedule of the tick number: the same
/// every run.
fn drive(tick: Res<cs_source_app::core::SimTick>, mut q: Query<(&Driven, &mut Intent)>) {
    for (d, mut i) in &mut q {
        let t = tick.0 + d.0 as u64 * 37;
        let phase = (t / 64) % 4;
        i.move_axis = [Vec2::Y, Vec2::X, -Vec2::Y, -Vec2::X][phase as usize];
        i.yaw = (t % 360) as f32 * 0.0174533;
        i.jump = t.is_multiple_of(97);
    }
}
