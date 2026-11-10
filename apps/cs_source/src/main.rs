//! The game binary (`cs_source`): the app's `APP` (lib.rs: CS:S's
//! simulation, map, network and client shell with its HUD and menus) on
//! Chimera's client app (`client::app::run`: the command line, the start
//! map, the plugins in order, then the per-frame schedules, main and
//! render world, on single-threaded executors, `CHIMERA_EXECUTOR=multi`:
//! Bevy's default, docs/performance.md; `CHIMERA_SYSTEM_DUMP`). Options:
//! `--help`.

fn main() {
    cs_source_app::client::app::run(&cs_source_app::APP);
}
