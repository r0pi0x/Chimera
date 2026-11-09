//! Whether Combat Arms' bots play on every game's guns and maps (user's
//! playtest: "some bots playing HL2:DM get stuck, don't shoot or move
//! properly"): six bots climbing a ladder of one game's guns on de_dust2 and
//! on dm_lockdown (which ships no `.nav`: the mesh is generated, as on
//! every map without AI volumes), for 40 s. Measured for each bot: the
//! ground it covers and the rounds it fires; a table with `--nocapture`.
//! Before the fixes (a foreign gun fired once a second, `bots::brain`; the
//! mesh walker narrower than a Source hull, `BotSettings::walker_radius`) an
//! HL2:DM SMG1's bots stood still in minute-long fights or pressed on into
//! gaps (0.2-11 m covered in 40 s); the map made no difference. Passes with
//! a note without the installs.

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::{
    minigame::{self, MinigameAppExt},
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{game::bots::CaBot, harness::CaSim};
use party_kit::PartyPartsPlugin;

/// The party on `map` with six bots on a ladder of `guns`; None without the
/// installs.
fn playing(map: &str, guns: &[&str]) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let usable = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"))
        .is_some()
        && config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game")).is_some()
        && hl2dm::game::INFO.install().is_some();
    if !usable {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
        return None;
    }
    let map = party_kit::load_map(map, 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    let text = originals::MINIGAMES.iter().find(|(f, _)| f.contains("gun_game_dust2")).unwrap().1;
    let start = text.find("ladder = [").unwrap();
    let end = start + text[start..].find(']').unwrap() + 1;
    let ladder = format!("ladder = [{}]", guns.iter().map(|g| format!("\"{g}\"")).collect::<Vec<_>>().join(", "));
    let text = format!("{}{}{}", &text[..start], ladder, &text[end..])
        .replace("originals:gun_game_dust2", "test:bots_on_rungs")
        .replace("weapons = [\"cs_source\", \"combat_arms\"]", "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]")
        .replace(
            "kit = { cs_source = [\"cs_source:weapon_knife\"], combat_arms = [\"combat_arms:m9_knife\"] }",
            "kit = { cs_source = [\"cs_source:weapon_knife\"], hl2dm = [\"hl2dm:weapon_crowbar\"], combat_arms = [\"combat_arms:m9_knife\"] }",
        );
    sim.app.add_minigames(&[("bots_on_rungs.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:bots_on_rungs").expect("the gun game starts");
    let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
    s.quota = 6;
    s.quota_set = true;
    Some(sim)
}

/// Rounds fired, by shooter.
#[derive(Resource, Default)]
struct Rounds(HashMap<Entity, u32>);

fn count_rounds(mut events: MessageReader<WeaponEvent>, mut rounds: ResMut<Rounds>) {
    for e in events.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. }) {
            *rounds.0.entry(e.owner).or_default() += 1;
        }
    }
}

/// Each bot's (metres covered, rounds fired) over `seconds`.
fn measure(sim: &mut Sim, seconds: f32) -> Vec<(f32, u32)> {
    sim.app.init_resource::<Rounds>().add_systems(FixedUpdate, count_rounds);
    // The bots in (one a tick), the navigation mesh made.
    sim.seconds(6.0);
    let mut walked: HashMap<Entity, (Vec3, f32)> = HashMap::new();
    for _ in 0..(seconds * 4.0) as u32 {
        sim.seconds(0.25);
        let w = sim.app.world_mut();
        for (e, t) in w.query_filtered::<(Entity, &Transform), With<CaBot>>().iter(w) {
            let row = walked.entry(e).or_insert((t.translation, 0.0));
            let step = t.translation.distance(row.0);
            // Not a respawn's jump.
            if step < 6.0 {
                row.1 += step;
            }
            row.0 = t.translation;
        }
    }
    let rounds = sim.app.world().resource::<Rounds>();
    walked.iter().map(|(e, (_, d))| (*d, rounds.0.get(e).copied().unwrap_or(0))).collect()
}

const CS: &[&str] = &["cs_source:weapon_ak47"];
const HL2: &[&str] = &[hl2dm::game::weapons::SMG1];

#[test]
fn bots_move_and_shoot_on_every_games_guns_and_maps() {
    let mut rows = Vec::new();
    for (map, guns) in [("cs_source:de_dust2", CS), ("hl2dm:dm_lockdown", CS), ("cs_source:de_dust2", HL2), ("hl2dm:dm_lockdown", HL2)] {
        let Some(mut sim) = playing(map, guns) else { return };
        let bots = measure(&mut sim, 40.0);
        let mesh = sim.app.world().get_resource::<chimera_world::map::nav::NavMesh>().map_or(0, |m| m.areas.len());
        rows.push((map, guns[0], mesh, bots));
    }
    println!("map, gun, nav areas, each bot's (m covered, rounds fired) in 40 s");
    for (map, gun, mesh, bots) in &rows {
        println!("{map:20} {gun:26} {mesh:5}  {bots:?}");
    }
    for (map, gun, mesh, bots) in &rows {
        assert!(*mesh > 0, "{map}: bots have navigation");
        assert_eq!(bots.len(), 6, "{map} {gun}");
        let shooting = bots.iter().filter(|b| b.1 > 0).count();
        assert!(shooting >= 3, "{map} {gun}: most bots fire: {bots:?}");
        let moving = bots.iter().filter(|b| b.0 > 20.0).count();
        assert!(moving >= 4, "{map} {gun}: most bots get about: {bots:?}");
    }
}
